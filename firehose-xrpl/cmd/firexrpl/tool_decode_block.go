package main

import (
	"encoding/hex"
	"fmt"
	"os"

	"github.com/spf13/cobra"
	"github.com/streamingfast/cli/sflags"
	pbxrpl "github.com/xrpl-commons/firehose-xrpl/pb/sf/xrpl/type/v1"
	"google.golang.org/protobuf/proto"
)

func NewToolDecodeBlockCmd() *cobra.Command {
	cmd := &cobra.Command{
		Use:   "tool-decode-block <block-file>",
		Short: "Decode and display an XRPL block from a .dbin file",
		Long: `Reads a Firehose block file (.dbin) and decodes the XRPL block,
displaying its contents in a human-readable format.

Example:
  firexrpl tool-decode-block /data/blocks/32570.dbin
`,
		Args: cobra.ExactArgs(1),
		RunE: runToolDecodeBlock,
	}

	cmd.Flags().Bool("show-transactions", true, "Show transaction details")
	cmd.Flags().Bool("show-raw", false, "Show raw hex blobs")

	return cmd
}

func runToolDecodeBlock(cmd *cobra.Command, args []string) error {
	blockFile := args[0]
	showTransactions := sflags.MustGetBool(cmd, "show-transactions")
	showRaw := sflags.MustGetBool(cmd, "show-raw")

	// Read the block file
	data, err := os.ReadFile(blockFile)
	if err != nil {
		return fmt.Errorf("reading block file: %w", err)
	}

	// Decode the block
	block := &pbxrpl.Block{}
	if err := proto.Unmarshal(data, block); err != nil {
		return fmt.Errorf("unmarshaling block: %w", err)
	}

	// Display block info
	fmt.Printf("=== XRPL Block ===\n")
	fmt.Printf("Ledger Index: %d\n", block.Number)
	fmt.Printf("Ledger Hash:  %s\n", hex.EncodeToString(block.Hash))
	fmt.Printf("Close Time:   %s\n", block.CloseTime.AsTime())
	fmt.Printf("Version:      %d\n", block.Version)
	fmt.Printf("Transactions: %d\n", len(block.Transactions))

	if block.Header != nil {
		fmt.Printf("\n=== Header ===\n")
		fmt.Printf("Parent Hash:          %s\n", hex.EncodeToString(block.Header.ParentHash))
		fmt.Printf("Total Drops:          %d\n", block.Header.TotalDrops)
		fmt.Printf("Account Hash:         %s\n", hex.EncodeToString(block.Header.AccountHash))
		fmt.Printf("Transaction Hash:     %s\n", hex.EncodeToString(block.Header.TransactionHash))
		fmt.Printf("Close Time Resolution: %d\n", block.Header.CloseTimeResolution)
		fmt.Printf("Close Flags:          %d\n", block.Header.CloseFlags)
	}

	if showTransactions && len(block.Transactions) > 0 {
		fmt.Printf("\n=== Transactions ===\n")
		for i, tx := range block.Transactions {
			fmt.Printf("\n--- Transaction %d ---\n", i)
			fmt.Printf("Hash:     %s\n", hex.EncodeToString(tx.Hash))
			fmt.Printf("Index:    %d\n", tx.Index)
			fmt.Printf("Type:     %s\n", tx.TxType)
			fmt.Printf("Result:   %s\n", tx.Result)
			fmt.Printf("Account:  %s\n", tx.Account)
			fmt.Printf("Fee:      %d drops\n", tx.Fee)
			fmt.Printf("Sequence: %d\n", tx.Sequence)

			if showRaw {
				fmt.Printf("TxBlob:   %s\n", hex.EncodeToString(tx.TxBlob))
				fmt.Printf("MetaBlob: %s\n", hex.EncodeToString(tx.MetaBlob))
			}
		}
	}

	return nil
}
