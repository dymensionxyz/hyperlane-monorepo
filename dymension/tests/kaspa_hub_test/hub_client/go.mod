module github.com/dymensionxyz/hub-client

go 1.23.1

require (
	github.com/cosmos/cosmos-sdk v0.50.13
	github.com/mitchellh/go-homedir v1.1.0
	github.com/spf13/cobra v1.8.0
	github.com/spf13/viper v1.18.1
	go.uber.org/zap v1.24.0
	github.com/bcp-innovations/hyperlane-cosmos v1.0.0
	cosmossdk.io/api v0.7.6
	cosmossdk.io/client/v2 v2.0.0-beta.8
	cosmossdk.io/collections v0.4.0
	cosmossdk.io/core v0.11.0
	cosmossdk.io/depinject v1.1.0
	cosmossdk.io/errors v1.0.1
	cosmossdk.io/log v1.4.1
	cosmossdk.io/math v1.4.0
	cosmossdk.io/store v1.1.1
)

replace (
	// github.com/evmos/evmos/v12 => github.com/dymensionxyz/evmos/v12 v12.1.6-dymension-v0.4.2
	github.com/gogo/protobuf => github.com/regen-network/protobuf v1.3.3-alpha.regen.1
	github.com/tendermint/tendermint => github.com/dymensionxyz/cometbft v0.34.29-0.20240807121422-5299b866061c
	github.com/bcp-innovations/hyperlane-cosmos => github.com/dymensionxyz/hyperlane-cosmos v0.0.0-20250611094246-7e116f7ab4f4
)
