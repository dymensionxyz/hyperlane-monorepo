#!/usr/bin/env node
// Debug script to trace the SmartProvider error
import { Router__factory, HypERC20Memo__factory, HypERC20__factory } from '@hyperlane-xyz/core';
import { MultiProvider, EvmERC20WarpRouteReader } from '@hyperlane-xyz/sdk';
import { ProtocolType } from '@hyperlane-xyz/utils';

async function debugProviderError() {
  const ETHEREUM_DYM_CONTRACT = process.env.ETHEREUM_DYM_CONTRACT || '0x626991CA73756C1c704b3B1D567634FC97936CB6';

  console.log(`\n=== Debugging SmartProvider Error for ${ETHEREUM_DYM_CONTRACT} ===\n`);

  // Setup chain metadata with extra logging
  const chainMetadata = {
    sepolia: {
      name: 'sepolia',
      chainId: 11155111,
      domainId: 11155111,
      protocol: ProtocolType.Ethereum,
      rpcUrls: [
        { http: 'https://ethereum-sepolia-rpc.publicnode.com' },
        { http: 'https://sepolia.drpc.org' },
        { http: 'https://rpc.sepolia.org' }
      ],
      nativeToken: {
        name: 'Ether',
        symbol: 'ETH',
        decimals: 18,
      },
    },
  };

  const multiProvider = new MultiProvider(chainMetadata);
  const chain = 'sepolia';
  const provider = multiProvider.getProvider(chain);

  // Enable debug logging on SmartProvider
  if ('setLogLevel' in provider) {
    console.log('Setting SmartProvider log level to debug...');
    provider.setLogLevel('debug');
  }

  try {
    // Step 1: Try to determine token type with detailed error catching
    console.log('1. Creating EvmERC20WarpRouteReader...');
    const reader = new EvmERC20WarpRouteReader(multiProvider, chain);

    console.log('\n2. Checking if contract exists...');
    const code = await provider.getCode(ETHEREUM_DYM_CONTRACT);
    console.log(`   Contract code length: ${code.length}`);

    console.log('\n3. Testing different contract interfaces to trigger the error...');

    // Test 1: Try to call a method that likely doesn't exist
    console.log('   a) Testing syntheticMemo interface...');
    try {
      const memoContract = HypERC20Memo__factory.connect(ETHEREUM_DYM_CONTRACT, provider);
      // Try to call transferRemoteMemo - this should fail if it's not a memo token
      const tx = await memoContract.populateTransaction.transferRemoteMemo(
        11155111,
        '0x' + '0'.repeat(64),
        0,
        '0x'
      );
      console.log('      ✓ Contract has transferRemoteMemo method');
    } catch (error) {
      console.log('      ✗ Error calling transferRemoteMemo:', error.code, error.reason || error.message);
    }

    // Test 2: Try synthetic interface
    console.log('   b) Testing synthetic interface...');
    try {
      const syntheticContract = HypERC20__factory.connect(ETHEREUM_DYM_CONTRACT, provider);
      const decimals = await syntheticContract.decimals();
      console.log(`      ✓ Contract has decimals: ${decimals}`);
    } catch (error) {
      console.log('      ✗ Error calling decimals:', error.code, error.reason || error.message);
    }

    // Test 3: Try router.domains() which should work
    console.log('   c) Testing router interface...');
    try {
      const router = Router__factory.connect(ETHEREUM_DYM_CONTRACT, provider);
      const domains = await router.domains();
      console.log(`      ✓ Contract has domains: ${domains.length > 0 ? domains.join(', ') : 'EMPTY'}`);
    } catch (error) {
      console.log('      ✗ Error calling domains:', error.code, error.reason || error.message);
    }

    // Step 4: Now try the actual deriveTokenType to see the full error flow
    console.log('\n4. Calling deriveTokenType (this triggers the warning)...');
    console.log('   Watch for "Unhandled error case" warning above this line:');
    console.log('   ---');

    try {
      const tokenType = await reader.deriveTokenType(ETHEREUM_DYM_CONTRACT);
      console.log(`   ---`);
      console.log(`   Token type successfully derived: ${tokenType}`);
    } catch (error) {
      console.log(`   ---`);
      console.log(`   Error deriving token type:`, error.message);
      console.log(`   Error code:`, error.code);
      console.log(`   Error cause:`, error.cause);
    }

    // Step 5: Check what's in remoteRouters
    console.log('\n5. Reading full warp route config...');
    const config = await reader.deriveWarpRouteConfig(ETHEREUM_DYM_CONTRACT);
    console.log('   Config type:', config.type);
    console.log('   Remote routers:', JSON.stringify(config.remoteRouters || {}, null, 2));
    console.log('   Destination gas:', JSON.stringify(config.destinationGas || {}, null, 2));

  } catch (error) {
    console.error('\n❌ Fatal error:', error);
  }
}

// Run with detailed error handling
debugProviderError().catch(error => {
  console.error('Script failed:', error);
  process.exit(1);
});