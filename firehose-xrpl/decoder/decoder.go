package decoder

import (
	"encoding/hex"
	"fmt"
	"sync"

	binarycodec "github.com/Peersyst/xrpl-go/binary-codec"
	xrpltx "github.com/Peersyst/xrpl-go/xrpl/transaction"
	pbxrpl "github.com/xrpl-commons/firehose-xrpl/pb/sf/xrpl/type/v1"
	"go.uber.org/zap"
)

// Decoder handles XRPL binary format decoding using xrpl-go's binarycodec
type Decoder struct {
	logger *zap.Logger
	mapper *Mapper
}

// NewDecoder creates a new XRPL decoder
func NewDecoder(logger *zap.Logger) *Decoder {
	return &Decoder{
		logger: logger,
		mapper: NewMapper(logger),
	}
}

// DecodeTransactionFromHex decodes a transaction blob (hex string) to a FlatTransaction
func (d *Decoder) DecodeTransactionFromHex(txBlobHex string) (xrpltx.FlatTransaction, error) {
	decoded, err := binarycodec.Decode(txBlobHex)
	if err != nil {
		return nil, fmt.Errorf("failed to decode transaction blob: %w", err)
	}

	return decoded, nil
}

// DecodeTransactionFromBytes decodes a transaction from raw bytes
func (d *Decoder) DecodeTransactionFromBytes(txBlob []byte) (xrpltx.FlatTransaction, error) {
	hexStr := hex.EncodeToString(txBlob)
	return d.DecodeTransactionFromHex(hexStr)
}

// DecodeMetadataFromHex decodes transaction metadata (hex string)
func (d *Decoder) DecodeMetadataFromHex(metaHex string) (map[string]interface{}, error) {
	decoded, err := binarycodec.Decode(metaHex)
	if err != nil {
		return nil, fmt.Errorf("failed to decode metadata: %w", err)
	}

	return decoded, nil
}

// DecodeMetadataFromBytes decodes metadata from raw bytes
func (d *Decoder) DecodeMetadataFromBytes(metaBlob []byte) (map[string]interface{}, error) {
	hexStr := hex.EncodeToString(metaBlob)
	return d.DecodeMetadataFromHex(hexStr)
}

// GetTransactionType extracts the transaction type string from a tx blob
func (d *Decoder) GetTransactionType(txBlob []byte) string {
	decoded, err := d.DecodeTransactionFromBytes(txBlob)
	if err != nil {
		d.logger.Debug("failed to decode transaction for type extraction", zap.Error(err))
		return ""
	}

	if txType, ok := decoded["TransactionType"].(string); ok {
		return txType
	}

	return ""
}

// GetTransactionResult extracts the result code string from metadata
func (d *Decoder) GetTransactionResult(metaBlob []byte) string {
	decoded, err := d.DecodeMetadataFromBytes(metaBlob)
	if err != nil {
		d.logger.Debug("failed to decode metadata for result extraction", zap.Error(err))
		return ""
	}

	if result, ok := decoded["TransactionResult"].(string); ok {
		return result
	}

	return ""
}

// MapTransactionToProto converts a decoded FlatTransaction and metadata to protobuf
// This is the main entry point used by the fetcher
// Accepts hex strings directly to avoid unnecessary encoding round-trips
func (d *Decoder) MapTransactionToProto(txBlobHex, metaBlobHex string, txHash []byte, txIndex uint32) (*pbxrpl.Transaction, error) {
	// Decode the transaction and metadata in parallel
	var flatTx xrpltx.FlatTransaction
	var meta map[string]interface{}
	var txErr, metaErr error
	var wg sync.WaitGroup

	wg.Add(2)

	go func() {
		defer wg.Done()
		flatTx, txErr = d.DecodeTransactionFromHex(txBlobHex)
	}()

	go func() {
		defer wg.Done()
		meta, metaErr = d.DecodeMetadataFromHex(metaBlobHex)
	}()

	wg.Wait()

	// Check for errors
	if txErr != nil {
		return nil, fmt.Errorf("decoding transaction: %w", txErr)
	}
	if metaErr != nil {
		return nil, fmt.Errorf("decoding metadata: %w", metaErr)
	}

	result := ""
	if txResult, ok := meta["TransactionResult"].(string); ok {
		result = txResult
	}

	// Decode hex to bytes for mapper (done once here instead of in fetcher + here)
	txBlob, err := hex.DecodeString(txBlobHex)
	if err != nil {
		return nil, fmt.Errorf("decoding tx blob hex: %w", err)
	}
	metaBlob, err := hex.DecodeString(metaBlobHex)
	if err != nil {
		return nil, fmt.Errorf("decoding meta blob hex: %w", err)
	}

	// Use the mapper to convert to protobuf
	return d.mapper.MapTransactionToProto(flatTx, txBlob, metaBlob, txHash, txIndex, result)
}
