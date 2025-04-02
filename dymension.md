How to test:
yarn test:forge --match-contract HypERC20MemoTest
yarn test:forge --match-contract HypERC20CollateralMemoTest
yarn test:forge --match-contract HypNativeMemoTest

Change list
Copied HypERC20 and modified to include memo in transferFromSender
Copied test for HypERC20 and added memo check
Copied HypNative and modified to include memo in transferFromSender
Copied test for HypNative and added memo check
Extended HypERC20Collateral with override to include memo in transferFromSender
Copied test for HypeERC20Collateral and added memo check

Improvements to be made (possibly) - DRY out repetitive memo code in contracts - DRY out repetitive new tests
