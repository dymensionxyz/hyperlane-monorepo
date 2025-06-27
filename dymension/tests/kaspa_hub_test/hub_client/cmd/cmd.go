package cmd

import (
	"fmt"
	"log"
	"os"

	"github.com/spf13/cobra"
	"github.com/spf13/viper"
	"go.uber.org/zap"
	"go.uber.org/zap/zapcore"

	"github.com/dymensionxyz/hub-client/logics"
	"github.com/dymensionxyz/hub-client/version"
)

/*
Actual business logic
*/
var cmdSetup = &cobra.Command{
	Use:   "setup",
	Short: "Run setup of Hyperlane objects on Hub",
	Run: func(cmd *cobra.Command, args []string) {
		viper.AutomaticEnv()

		if err := viper.ReadInConfig(); err == nil {
			fmt.Println("Using config file:", viper.ConfigFileUsed())
		}

		cfg := logics.Config{}
		if err := viper.Unmarshal(&cfg); err != nil {
			log.Fatalf("failed to unmarshal config: %v", err)
		}

		log.Printf("using config file: %+v", viper.ConfigFileUsed())

		logger, err := buildLogger(cfg.LogLevel)
		if err != nil {
			log.Fatalf("failed to build logger: %v", err)
		}

		// Ensure all logs are written
		defer logger.Sync() // nolint: errcheck
	},
}

var RootCmd = &cobra.Command{
	Use:   "hub-client",
	Short: "Setup Hyperlane on the Hub for Kaspa test",
	Run: func(cmd *cobra.Command, args []string) {
		// If no arguments are provided, print usage information
		if len(args) == 0 {
			if err := cmd.Usage(); err != nil {
				log.Fatalf("Error printing usage: %v", err)
			}
		}
	},
}

var cmdInit = &cobra.Command{
	Use:   "init",
	Short: "init a config file",
	Run: func(cmd *cobra.Command, args []string) {
		cfg := logics.Config{}
		if err := viper.Unmarshal(&cfg); err != nil {
			log.Fatalf("failed to unmarshal config: %v", err)
		}

		if err := viper.WriteConfigAs(logics.CfgFile); err != nil {
			log.Fatalf("failed to write config file: %v", err)
		}

		fmt.Printf("Config file created: %s\n", logics.CfgFile)
		fmt.Println()
		fmt.Println("Edit the config file to set the correct values for your environment.")
	},
}

func buildLogger(logLevel string) (*zap.Logger, error) {
	var level zapcore.Level
	if err := level.Set(logLevel); err != nil {
		return nil, fmt.Errorf("failed to set log level: %w", err)
	}

	encoderConfig := zap.NewProductionEncoderConfig()
	encoderConfig.EncodeTime = zapcore.ISO8601TimeEncoder

	logger := zap.New(zapcore.NewCore(
		zapcore.NewJSONEncoder(encoderConfig),
		zapcore.Lock(os.Stdout),
		level,
	))

	return logger, nil
}

var cmdVer = &cobra.Command{
	Use:   "version",
	Short: "Print the version of eibc-client",
	Run: func(cmd *cobra.Command, args []string) {
		fmt.Println(version.BuildVersion)
	},
}

func init() {
	RootCmd.CompletionOptions.DisableDefaultCmd = true
	RootCmd.AddCommand(cmdInit)
	RootCmd.AddCommand(cmdSetup)
	RootCmd.AddCommand(cmdVer)

	cobra.OnInitialize(logics.InitConfig)

	RootCmd.PersistentFlags().StringVar(&logics.CfgFile, "config", "", "config file")

	// Cobra also supports local flags, which will only run
	// when this action is called directly.
	RootCmd.Flags().BoolP("toggle", "t", false, "Help message for toggle")
}
