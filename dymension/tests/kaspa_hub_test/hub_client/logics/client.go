package logics

import (
	"fmt"
	"log"
	"time"

	sdk "github.com/cosmos/cosmos-sdk/types"
	"github.com/dymensionxyz/cosmosclient/cosmosclient"
	"go.uber.org/zap"
)

type bridgeClient struct {
	logger *zap.Logger
	config Config
}

func NewOrderClient(cfg Config, logger *zap.Logger) (*bridgeClient, error) {
	sdkcfg := sdk.GetConfig()
	sdkcfg.SetBech32PrefixForAccount(HubAddressPrefix, PubKeyPrefix)

	hubClient, err := getHubClient(cfg)
	if err != nil {
		return nil, fmt.Errorf("failed to create hub client: %w", err)
	}

	return &bridgeClient{
		logger: logger,
		config: cfg,
	}, nil
}

const (
	connectAttempts = 5
	connectSleep    = 10 * time.Second
)

func getHubClient(cfg Config) (hubClient cosmosclient.Client, err error) {
	// init cosmos client for order fetcher
	hubClientCfg := ClientConfig{
		HomeDir:        cfg.Fulfillers.KeyringDir,
		NodeAddress:    cfg.NodeAddress,
		GasFees:        cfg.Gas.Fees,
		GasPrices:      cfg.Gas.Prices,
		KeyringBackend: cfg.Fulfillers.KeyringBackend,
	}

	err = retry(connectAttempts, connectSleep, func() error {
		var retryErr error
		hubClient, retryErr = cosmosclient.New(config.GetCosmosClientOptions(hubClientCfg)...)
		if retryErr != nil {
			log.Printf("failed to obtain hub client, retrying in 10 seconds: %s", retryErr.Error())
		}
		return retryErr
	})
	if err != nil {
		return cosmosclient.Client{}, fmt.Errorf("failed to create cosmos client after retries: %w", err)
	}

	return hubClient, nil
}

func retry(attempts int, sleep time.Duration, f func() error) (err error) {
	for i := 0; i < attempts; i++ {
		err = f()
		if err == nil {
			return
		}
		if i < attempts-1 {
			time.Sleep(sleep)
		}
	}
	return fmt.Errorf("all retry attempts failed: %w", err)
}
