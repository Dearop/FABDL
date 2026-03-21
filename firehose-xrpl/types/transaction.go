package types

// TxRequest represents a request to fetch a specific transaction
type TxRequest struct {
	Method string     `json:"method"`
	Params []TxParams `json:"params"`
}

type TxParams struct {
	Transaction string `json:"transaction"`
	Binary      bool   `json:"binary,omitempty"`
}

// TxResponse represents the response from the tx method
type TxResponse struct {
	Result TxResult `json:"result"`
}

type TxResult struct {
	Hash            string `json:"hash"`
	LedgerIndex     uint64 `json:"ledger_index"`
	Status          string `json:"status"`
	Validated       bool   `json:"validated"`
	Meta            any    `json:"meta,omitempty"`      // JSON or binary
	MetaBlob        string `json:"meta_blob,omitempty"` // When binary=true
	TxBlob          string `json:"tx_blob,omitempty"`   // When binary=true
	TransactionType string `json:"TransactionType,omitempty"`
	Account         string `json:"Account,omitempty"`
	Fee             string `json:"Fee,omitempty"`
	Sequence        uint32 `json:"Sequence,omitempty"`
}
