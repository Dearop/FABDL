package main

import (
	"github.com/spf13/cobra"
	"github.com/streamingfast/cli"
	. "github.com/streamingfast/cli"
	"github.com/streamingfast/logging"
	"go.uber.org/zap"
)

// Injected at build time
var version = "<missing>"

var logger, tracer = logging.PackageLogger("firexrpl", "github.com/xrpl-commons/firehose-xrpl")

func main() {
	logging.InstantiateLoggers(logging.WithDefaultLevel(zap.InfoLevel))

	Run(
		"firexrpl",
		"Firehose XRPL block fetching and tooling",
		Description(`
			Firehose XRPL implements the Firehose Reader protocol for XRP Ledger,
			via 'firexrpl fetch rpc <flags>' (see 'firexrpl fetch rpc --help').

			It is expected to be used with the Firehose Stack by operating 'firecore'
			binary which spawns Firehose XRPL Reader as a subprocess and reads from
			it producing blocks and offering Firehose & Substreams APIs.

			Read the Firehose documentation at firehose.streamingfast.io for more
			information on how to use this binary.

			The binary also contains utility tools to test XRPL ledger
			fetching capabilities.

			XRPL Endpoints:
			  Mainnet: https://s1.ripple.com:51234/ or https://xrplcluster.com/
			  Testnet: https://s.altnet.rippletest.net:51234/
			  Devnet:  https://s.devnet.rippletest.net:51234/
		`),

		ConfigureVersion(version),
		ConfigureViper("FIREXRPL"),

		Group("fetch", "Reader Node fetch RPC command",
			CobraCmd(NewFetchCmd(logger, tracer)),
		),

		CobraCmd(NewToolDecodeBlockCmd()),
		CobraCmd(NewToolCheckLedgerCmd()),

		OnCommandErrorLogAndExit(logger),
	)
}

func CobraCmd(cmd *cobra.Command) cli.CommandOption {
	return cli.CommandOptionFunc(func(parent *cobra.Command) {
		parent.AddCommand(cmd)
	})
}
