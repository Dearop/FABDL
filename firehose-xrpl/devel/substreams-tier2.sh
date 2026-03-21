firecore \
    start \
    substreams-tier2 \
    --config-file= \
    --log-format=stackdriver \
    --advertise-chain-name=xrpl \
    --advertise-block-id-encoding=hex \
    --common-merged-blocks-store-url=data/storage/merged-blocks \
    --common-first-streamable-block=80000000 \
    --common-one-block-store-url=data/oneblock \
    --substreams-tier1-grpc-listen-addr=9000 \
    --substreams-tier2-grpc-listen-addr=:9001 \
    --substreams-state-store-url=data/substreams-tier2/states \
    --substreams-state-bundle-size=100
