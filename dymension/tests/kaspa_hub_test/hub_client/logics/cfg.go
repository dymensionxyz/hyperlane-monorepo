package logics

import (
	"log"

	"github.com/cosmos/cosmos-sdk/client"
	"github.com/dymensionxyz/cosmosclient/cosmosclient"
	"github.com/ignite/cli/ignite/pkg/cosmosaccount"
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
	NodeAddress    string                       `mapstructure:"node_address"`
	Gas            GasConfig                    `mapstructure:"gas"`
	KeyringBackend cosmosaccount.KeyringBackend `mapstructure:"keyring_backend"`
	KeyringDir     string                       `mapstructure:"keyring_dir"`
	LogLevel       string                       `mapstructure:"log_level"`
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
	HomeDir        string
	NodeAddress    string
	GasFees        string
	GasPrices      string
	FeeGranter     string
	KeyringBackend cosmosaccount.KeyringBackend
}

func GetCosmosClientOptions(config ClientConfig) []cosmosclient.Option {
	options := []client.Option{
		cosmosclient.WithAddressPrefix(HubAddressPrefix),
		cosmosclient.WithHome(config.HomeDir),
		cosmosclient.WithNodeAddress(config.NodeAddress),
		cosmosclient.WithFees(config.GasFees),
		cosmosclient.WithGas(cosmosclient.GasAuto),
		cosmosclient.WithGasPrices(config.GasPrices),
		cosmosclient.WithGasAdjustment(1.3),
		cosmosclient.WithKeyringBackend(config.KeyringBackend),
		cosmosclient.WithKeyringDir(config.HomeDir),
		cosmosclient.WithFeeGranter(config.FeeGranter),
	}
	return options
}
