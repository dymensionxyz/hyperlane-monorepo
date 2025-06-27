package logics

import (
	"log"

	"github.com/mitchellh/go-homedir"
	"github.com/spf13/viper"
)

const (
	defaultNodeAddress = "http://localhost:36657"
	HubAddressPrefix   = "dym"
	defaultLogLevel    = "info"
	defaultHubDenom    = "adym"
	defaultGasFees     = "3000000000000000" + defaultHubDenom
	testKeyringBackend = "test"
	PubKeyPrefix       = "pub"
)

type Config struct {
	NodeAddress string    `mapstructure:"node_address"`
	Gas         GasConfig `mapstructure:"gas"`
	// KeyringBackend account.KeyringBackend `mapstructure:"keyring_backend"`
	KeyringDir string `mapstructure:"keyring_dir"`
	LogLevel   string `mapstructure:"log_level"`
}

type GasConfig struct {
	Prices string `mapstructure:"prices"`
	Fees   string `mapstructure:"fees"`
}

const dir = "/.dymension-hyperlane-kaspa-hub-setup"

func InitConfig() {
	// Set default values
	// Find home directory.
	home, err := homedir.Dir()
	if err != nil {
		log.Fatalf("failed to get home directory: %v", err)
	}
	defaultHomeDir := home + dir

	viper.SetDefault("log_level", defaultLogLevel)
	viper.SetDefault("node_address", defaultNodeAddress)
	viper.SetDefault("gas.fees", defaultGasFees)
	viper.SetDefault("keyring_backend", testKeyringBackend)
	viper.SetDefault("keyring_dir", defaultHomeDir)

	viper.SetConfigType("yaml")
	if CfgFile != "" {
		// Use config file from the flag.
		viper.SetConfigFile(CfgFile)
	} else {
		CfgFile = defaultHomeDir + "/config.yaml"
		viper.AddConfigPath(defaultHomeDir)
		viper.AddConfigPath(".")
		viper.SetConfigName("config")
	}
}

var CfgFile string

type ClientConfig struct {
	HomeDir     string
	NodeAddress string
	GasFees     string
	GasPrices   string
	FeeGranter  string
	// KeyringBackend account.KeyringBackend
}

// func GetCosmosClientOptions(config ClientConfig) []client.Option {
// 	options := []client.Option{
// 		client.WithAddressPrefix(HubAddressPrefix),
// 		client.WithHome(config.HomeDir),
// 		client.WithNodeAddress(config.NodeAddress),
// 		client.WithFees(config.GasFees),
// 		client.WithGas(client.GasAuto),
// 		client.WithGasPrices(config.GasPrices),
// 		client.WithGasAdjustment(1.3),
// 		client.WithKeyringBackend(config.KeyringBackend),
// 		client.WithKeyringDir(config.HomeDir),
// 		client.WithFeeGranter(config.FeeGranter),
// 	}
// 	return options
// }
