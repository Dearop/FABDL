package types

// RPCError represents a JSON-RPC error response from rippled
type RPCError struct {
	Error        string `json:"error"`
	ErrorCode    int    `json:"error_code"`
	ErrorMessage string `json:"error_message"`
	Status       string `json:"status"`
	Request      any    `json:"request,omitempty"`
}

func (r *RPCError) IsError() bool {
	return r.Error != "" || r.Status == "error"
}

// LedgerClosedRequest represents a request to get the latest closed ledger
type LedgerClosedRequest struct {
	Method string `json:"method"`
	Params []any  `json:"params"`
}

// LedgerClosedResponse represents the response from ledger_closed
type LedgerClosedResponse struct {
	Result LedgerClosedResult `json:"result"`
}

type LedgerClosedResult struct {
	LedgerHash  string `json:"ledger_hash"`
	LedgerIndex uint64 `json:"ledger_index"`
	Status      string `json:"status"`
	// Error fields (present when status == "error")
	Error        string `json:"error,omitempty"`
	ErrorCode    int    `json:"error_code,omitempty"`
	ErrorMessage string `json:"error_message,omitempty"`
}

// LedgerRequest represents a request to fetch a specific ledger with transactions
type LedgerRequest struct {
	Method string         `json:"method"`
	Params []LedgerParams `json:"params"`
}

type LedgerParams struct {
	LedgerIndex  any    `json:"ledger_index,omitempty"` // Can be uint64 or string ("validated", "closed", "current")
	LedgerHash   string `json:"ledger_hash,omitempty"`
	Transactions bool   `json:"transactions"`
	Expand       bool   `json:"expand"`
	Binary       bool   `json:"binary"`
	OwnerFunds   bool   `json:"owner_funds,omitempty"`
}

type LedgerOptions struct {
	Transactions bool
	Expand       bool
	Binary       bool
	OwnerFunds   bool
}

// LedgerResponse represents the response from the ledger method
type LedgerResponse struct {
	Result LedgerResult `json:"result"`
}

type LedgerResult struct {
	Ledger      Ledger `json:"ledger"`
	LedgerHash  string `json:"ledger_hash"`
	LedgerIndex uint64 `json:"ledger_index"`
	Validated   bool   `json:"validated"`
	Status      string `json:"status"`
	// Error fields
	Error        string `json:"error,omitempty"`
	ErrorCode    int    `json:"error_code,omitempty"`
	ErrorMessage string `json:"error_message,omitempty"`
}

// Ledger represents an XRPL ledger
type Ledger struct {
	// Core identifiers
	LedgerIndex uint64 `json:"ledger_index"`
	LedgerHash  string `json:"ledger_hash"`
	ParentHash  string `json:"parent_hash"`

	// Timestamps - XRPL uses seconds since Ripple Epoch (2000-01-01)
	CloseTime       uint64 `json:"close_time"`
	CloseTimeHuman  string `json:"close_time_human,omitempty"`
	ParentCloseTime uint64 `json:"parent_close_time"`

	// Hashes
	AccountHash     string `json:"account_hash"`
	TransactionHash string `json:"transaction_hash"`

	// Amounts (as strings to handle large numbers)
	TotalCoins string `json:"total_coins"` // Total XRP in drops

	// Ledger properties
	CloseTimeResolution uint32 `json:"close_time_resolution"`
	CloseFlags          uint32 `json:"close_flags,omitempty"`

	// Transactions (when transactions=true, expand=true)
	Transactions []LedgerTransaction `json:"transactions,omitempty"`

	// Closed/validated status
	Closed bool `json:"closed,omitempty"`
}

// LedgerTransaction represents a transaction in a ledger response
// When binary=true, TxBlob and Meta are hex strings
// When binary=false, they are JSON objects
type LedgerTransaction struct {
	// When binary=true
	Hash   string `json:"hash,omitempty"`
	TxBlob string `json:"tx_blob,omitempty"`
	Meta   string `json:"meta,omitempty"`

	// When binary=false (JSON format) or decoded from binary
	Account         string `json:"Account,omitempty"`
	TransactionType string `json:"TransactionType,omitempty"`
	Fee             string `json:"Fee,omitempty"`
	Sequence        uint32 `json:"Sequence,omitempty"`
	Destination     string `json:"Destination,omitempty"` // For Payment and similar transactions
	MetaData        any    `json:"metaData,omitempty"`    // JSON metadata when binary=false
}

// ServerInfoRequest represents a request to get server information
type ServerInfoRequest struct {
	Method string `json:"method"`
	Params []any  `json:"params"`
}

// ServerInfoResponse represents the response from server_info
type ServerInfoResponse struct {
	Result ServerInfoResult `json:"result"`
}

type ServerInfoResult struct {
	Info   ServerInfo `json:"info"`
	Status string     `json:"status"`
}

type ServerInfo struct {
	BuildVersion    string        `json:"build_version"`
	CompleteLedgers string        `json:"complete_ledgers"`
	HostID          string        `json:"hostid"`
	ServerState     string        `json:"server_state"`
	ValidatedLedger ValidatedInfo `json:"validated_ledger,omitempty"`
}

type ValidatedInfo struct {
	Age            uint32  `json:"age"`
	BaseFeeXRP     float64 `json:"base_fee_xrp"`
	Hash           string  `json:"hash"`
	ReserveBaseXRP float64 `json:"reserve_base_xrp"`
	ReserveIncXRP  float64 `json:"reserve_inc_xrp"`
	Seq            uint64  `json:"seq"`
}
