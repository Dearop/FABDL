package rpc

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"time"

	binarycodec "github.com/Peersyst/xrpl-go/binary-codec"
	"github.com/Peersyst/xrpl-go/xrpl/rpc"
	"github.com/xrpl-commons/firehose-xrpl/types"
	"go.uber.org/zap"
)

// Client wraps the xrpl-go RPC client for Firehose operations
type Client struct {
	rpcEndpoint string
	client      *rpc.Client
	httpClient  *http.Client
	logger      *zap.Logger
}

// NewClient creates a new XRPL RPC client with default HTTP settings
func NewClient(rpcEndpoint string, logger *zap.Logger) (*Client, error) {
	return NewClientWithHTTPConfig(rpcEndpoint, logger, 100, 10, 90*time.Second)
}

// NewClientWithHTTPConfig creates a new XRPL RPC client with custom HTTP connection pool settings
func NewClientWithHTTPConfig(rpcEndpoint string, logger *zap.Logger, maxIdleConns, maxIdleConnsPerHost int, idleConnTimeout time.Duration) (*Client, error) {
	cfg, err := rpc.NewClientConfig(rpcEndpoint,
		rpc.WithTimeout(60*time.Second),
	)
	if err != nil {
		return nil, fmt.Errorf("failed to create client config: %w", err)
	}

	client := rpc.NewClient(cfg)

	// Configure HTTP transport with connection pooling
	transport := &http.Transport{
		MaxIdleConns:          maxIdleConns,
		MaxIdleConnsPerHost:   maxIdleConnsPerHost,
		IdleConnTimeout:       idleConnTimeout,
		DisableCompression:    false,
		ForceAttemptHTTP2:     true,
		MaxConnsPerHost:       0, // No limit on total connections per host
		ResponseHeaderTimeout: 30 * time.Second,
		ExpectContinueTimeout: 1 * time.Second,
	}

	return &Client{
		rpcEndpoint: rpcEndpoint,
		client:      client,
		httpClient: &http.Client{
			Timeout:   60 * time.Second,
			Transport: transport,
		},
		logger: logger,
	}, nil
}

// GetLatestLedger returns the latest validated ledger index
func (c *Client) GetLatestLedger(ctx context.Context) (*types.LedgerClosedResult, error) {
	// Use GetClosedLedger to get the latest closed ledger
	response, err := c.client.GetClosedLedger()
	if err != nil {
		return nil, fmt.Errorf("ledger_closed request failed: %w", err)
	}

	return &types.LedgerClosedResult{
		LedgerHash:  response.LedgerHash,
		LedgerIndex: uint64(response.LedgerIndex),
		Status:      "success",
	}, nil
}

// rawLedgerResponse is the raw JSON response from rippled for binary mode
type rawLedgerResponse struct {
	Result struct {
		Ledger struct {
			LedgerData   string        `json:"ledger_data"`
			Closed       bool          `json:"closed"`
			Transactions []interface{} `json:"transactions"`
		} `json:"ledger"`
		LedgerHash  string `json:"ledger_hash"`
		LedgerIndex uint64 `json:"ledger_index"`
		Validated   bool   `json:"validated"`
		Status      string `json:"status"`
		Error       string `json:"error,omitempty"`
	} `json:"result"`
}

// GetLedger fetches a ledger with all transactions in binary format
func (c *Client) GetLedger(ctx context.Context, ledgerIndex uint64) (*types.LedgerResult, error) {
	startTime := time.Now()
	defer func() {
		c.logger.Debug("GetLedger completed",
			zap.Uint64("ledger_index", ledgerIndex),
			zap.Duration("duration", time.Since(startTime)))
	}()
	// Make raw HTTP request to get ledger_data blob which xrpl-go doesn't expose
	reqBody := fmt.Sprintf(`{"method":"ledger","params":[{"ledger_index":%d,"transactions":true,"expand":true,"binary":true}]}`, ledgerIndex)

	req, err := http.NewRequestWithContext(ctx, "POST", c.rpcEndpoint, bytes.NewBufferString(reqBody))
	if err != nil {
		return nil, fmt.Errorf("failed to create request: %w", err)
	}
	req.Header.Set("Content-Type", "application/json")

	resp, err := c.httpClient.Do(req)
	if err != nil {
		return nil, fmt.Errorf("ledger request failed: %w", err)
	}
	defer func(Body io.ReadCloser) {
		err := Body.Close()
		if err != nil {
			_ = fmt.Errorf("failed to close response body: %w", err)
		}
	}(resp.Body)

	// Stream JSON parsing - avoids buffering entire response in memory
	var rawResp rawLedgerResponse
	if err := json.NewDecoder(resp.Body).Decode(&rawResp); err != nil {
		return nil, fmt.Errorf("failed to parse response: %w", err)
	}

	if rawResp.Result.Error != "" {
		return nil, fmt.Errorf("RPC error: %s", rawResp.Result.Error)
	}

	if !rawResp.Result.Validated {
		return nil, fmt.Errorf("ledger %d not yet validated", ledgerIndex)
	}

	// Decode ledger header from ledger_data blob
	ledgerData := types.Ledger{
		LedgerIndex: rawResp.Result.LedgerIndex,
		LedgerHash:  rawResp.Result.LedgerHash,
		Closed:      rawResp.Result.Ledger.Closed,
	}

	if rawResp.Result.Ledger.LedgerData != "" {
		headerData, err := binarycodec.DecodeLedgerData(rawResp.Result.Ledger.LedgerData)
		if err != nil {
			c.logger.Warn("failed to decode ledger_data", zap.Error(err))
		} else {
			ledgerData.ParentHash = headerData.ParentHash
			ledgerData.CloseTime = uint64(headerData.CloseTime)
			ledgerData.ParentCloseTime = uint64(headerData.ParentCloseTime)
			ledgerData.AccountHash = headerData.AccountHash
			ledgerData.TransactionHash = headerData.TransactionHash
			ledgerData.TotalCoins = headerData.TotalCoins
			ledgerData.CloseTimeResolution = uint32(headerData.CloseTimeResolution)
			ledgerData.CloseFlags = uint32(headerData.CloseFlags)
		}
	}

	// Convert transactions - in binary mode we get tx_blob and meta
	if rawResp.Result.Ledger.Transactions != nil {
		ledgerData.Transactions = make([]types.LedgerTransaction, 0, len(rawResp.Result.Ledger.Transactions))
		for _, tx := range rawResp.Result.Ledger.Transactions {
			ltx := types.LedgerTransaction{}

			// Extract fields from transaction map
			if txMap, ok := tx.(map[string]interface{}); ok {
				// Get hash directly from response (more efficient than computing)
				if hash, ok := txMap["hash"].(string); ok {
					ltx.Hash = hash
				}
				// Get tx_blob
				if txBlob, ok := txMap["tx_blob"].(string); ok {
					ltx.TxBlob = txBlob
				}
				// Get meta (rippled uses "meta" in binary mode)
				if meta, ok := txMap["meta"].(string); ok {
					ltx.Meta = meta
				}
			}

			ledgerData.Transactions = append(ledgerData.Transactions, ltx)
		}
	}

	return &types.LedgerResult{
		Ledger:      ledgerData,
		LedgerHash:  rawResp.Result.LedgerHash,
		LedgerIndex: rawResp.Result.LedgerIndex,
		Validated:   rawResp.Result.Validated,
		Status:      "success",
	}, nil
}

// GetServerInfo returns server information including available ledger range
func (c *Client) GetServerInfo(ctx context.Context) (*types.ServerInfoResult, error) {
	// Use Ping to test connection - server_info not directly available
	// For now, we'll use GetLedgerIndex as a health check
	ledgerIndex, err := c.client.GetLedgerIndex()
	if err != nil {
		return nil, fmt.Errorf("server check failed: %w", err)
	}

	return &types.ServerInfoResult{
		Status: "success",
		Info: types.ServerInfo{
			ServerState: "connected",
			ValidatedLedger: types.ValidatedInfo{
				Seq: uint64(ledgerIndex),
			},
		},
	}, nil
}
