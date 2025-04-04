Testing
yarn test:forge --match-contract HypERC20MemoTest
yarn test:forge --match-contract HypERC20CollateralMemoTest
yarn test:forge --match-contract HypNativeMemoTest

Notes
Ethereum
HypERC20 = Synthetic
HypERC20Collateral = Collateral
HypNative = Native
Solana
hyperlane-sealevel-token = Synthetic
hyperlane-sealevel-token-collateral = Collateral
hyperlane-sealevel-token-native = Native (?)

Change list
Ethereum
Copied HypERC20 and modified to include memo in transferFromSender
Copied test for HypERC20 and added memo check
Copied HypNative and modified to include memo in transferFromSender
Copied test for HypNative and added memo check
Extended HypERC20Collateral with override to include memo in transferFromSender
Copied test for HypeERC20Collateral and added memo check
Solana

Improvements to be made (possibly)
DRY out repetitive memo code in contracts - DRY out repetitive new tests
