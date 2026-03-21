package main

import (
	"fmt"
	"strconv"
	"time"

	"github.com/spf13/cobra"
	"github.com/streamingfast/cli/sflags"
	firecore "github.com/streamingfast/firehose-core"
	"github.com/streamingfast/firehose-core/blockpoller"
	firecoreRPC "github.com/streamingfast/firehose-core/rpc"
	"github.com/streamingfast/logging"
	"github.com/xrpl-commons/firehose-xrpl/rpc"
	"go.uber.org/zap"
)

func NewFetchCmd(logger *zap.Logger, tracer logging.Tracer) *cobra.Command {
	cmd := &cobra.Command{
		Use:   "rpc <first-streamable-block>",
		Short: "Fetch blocks from XRPL RPC endpoint",
		Long: `Fetches XRPL ledgers from a rippled JSON-RPC endpoint and outputs them
in the Firehose block format.

The fetcher polls the RPC endpoint for new validated ledgers and converts
them to Firehose blocks. Unlike Stellar, XRPL returns transactions inline
with the ledger response, simplifying the fetch logic.

Example:
  firexrpl fetch rpc 32570 \
    --endpoints https://s1.ripple.com:51234/ \
    --state-dir /data/poller

XRPL Endpoints:
  Mainnet: https://s1.ripple.com:51234/
  Mainnet: https://xrplcluster.com/
  Testnet: https://s.altnet.rippletest.net:51234/
  Devnet:  https://s.devnet.rippletest.net:51234/
`,
		Args: cobra.ExactArgs(1),
		RunE: fetchRunE(logger, tracer),
	}

	cmd.Flags().StringArray("endpoints", []string{}, "List of XRPL RPC endpoints (comma-separated or multiple flags)")
	cmd.Flags().String("state-dir", "/data/poller", "Directory to store poller state")
	cmd.Flags().Duration("interval-between-fetch", 0, "Interval between consecutive fetches")
	cmd.Flags().Duration("latest-block-retry-interval", time.Second, "Interval to wait before retrying when waiting for new ledger")
	cmd.Flags().Duration("max-block-fetch-duration", 10*time.Second, "Maximum duration for fetching a single block")
	cmd.Flags().Int("block-fetch-batch-size", 1, "Number of blocks to fetch in a single batch")
	cmd.Flags().Int("worker-pool-size", 10, "Number of concurrent workers for processing transactions within a block")
	cmd.Flags().Int("http-max-idle-conns", 100, "Maximum number of idle HTTP connections in the pool")
	cmd.Flags().Int("http-max-idle-conns-per-host", 10, "Maximum number of idle HTTP connections per host")
	cmd.Flags().Duration("http-idle-conn-timeout", 90*time.Second, "Maximum time an idle connection is kept alive")

	return cmd
}

func fetchRunE(logger *zap.Logger, tracer logging.Tracer) firecore.CommandExecutor {
	return func(cmd *cobra.Command, args []string) (err error) {
		stateDir := sflags.MustGetString(cmd, "state-dir")

		startBlock, err := strconv.ParseUint(args[0], 10, 64)
		if err != nil {
			return fmt.Errorf("unable to parse first streamable block %d: %w", startBlock, err)
		}

		fetchInterval := sflags.MustGetDuration(cmd, "interval-between-fetch")
		latestBlockRetryInterval := sflags.MustGetDuration(cmd, "latest-block-retry-interval")
		maxBlockFetchDuration := sflags.MustGetDuration(cmd, "max-block-fetch-duration")

		logger.Info(
			"launching firehose-xrpl poller",
			zap.String("state_dir", stateDir),
			zap.Uint64("first_streamable_block", startBlock),
			zap.Duration("interval_between_fetch", fetchInterval),
			zap.Duration("latest_block_retry_interval", latestBlockRetryInterval),
			zap.Duration("max_block_fetch_duration", maxBlockFetchDuration),
		)

		rpcEndpoints := sflags.MustGetStringArray(cmd, "endpoints")
		if len(rpcEndpoints) == 0 {
			return fmt.Errorf("at least one --endpoints must be provided")
		}

		// Get HTTP connection pool settings
		httpMaxIdleConns := sflags.MustGetInt(cmd, "http-max-idle-conns")
		httpMaxIdleConnsPerHost := sflags.MustGetInt(cmd, "http-max-idle-conns-per-host")
		httpIdleConnTimeout := sflags.MustGetDuration(cmd, "http-idle-conn-timeout")

		// Create rolling strategy for RPC clients
		rollingStrategy := firecoreRPC.NewStickyRollingStrategy[*rpc.Client]()

		// Create RPC clients manager
		rpcClients := firecoreRPC.NewClients(maxBlockFetchDuration, rollingStrategy, logger)
		for _, endpoint := range rpcEndpoints {
			client, err := rpc.NewClientWithHTTPConfig(endpoint, logger, httpMaxIdleConns, httpMaxIdleConnsPerHost, httpIdleConnTimeout)
			if err != nil {
				return fmt.Errorf("failed to create client for endpoint %s: %w", endpoint, err)
			}
			rpcClients.Add(client)
			logger.Info("added RPC endpoint",
				zap.String("endpoint", endpoint),
				zap.Int("max_idle_conns", httpMaxIdleConns),
				zap.Int("max_idle_conns_per_host", httpMaxIdleConnsPerHost),
				zap.Duration("idle_conn_timeout", httpIdleConnTimeout))
		}

		workerPoolSize := sflags.MustGetInt(cmd, "worker-pool-size")
	fetcher := rpc.NewFetcherWithWorkerPool(fetchInterval, latestBlockRetryInterval, workerPoolSize, logger)

		poller := blockpoller.New(
			fetcher,
			blockpoller.NewFireBlockHandler("type.googleapis.com/sf.xrpl.type.v1.Block"),
			rpcClients,
			blockpoller.WithStoringState[*rpc.Client](stateDir),
			blockpoller.WithLogger[*rpc.Client](logger),
		)

		err = poller.Run(startBlock, nil, sflags.MustGetInt(cmd, "block-fetch-batch-size"))
		if err != nil {
			return fmt.Errorf("running poller: %w", err)
		}

		return nil
	}
}
