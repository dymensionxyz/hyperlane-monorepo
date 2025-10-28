#!/usr/bin/env node
// Debug script to test warp route reading with detailed logging
import { Router__factory, TokenRouter__factory } from '@hyperlane-xyz/core';
import { MultiProvider, EvmERC20WarpRouteReader } from '@hyperlane-xyz/sdk';
import { ProtocolType } from '@hyperlane-xyz/utils';

async function debugWarpRouteCLI() {
  // Get contract address from environment or use the known address
  const ETHEREUM_DYM_CONTRACT = process.env.ETHEREUM_DYM_CONTRACT || '0x626991CA73756C1c704b3B1D567634FC97936CB6';

  console.log(`\n=== CLI Debug for Warp Route ${ETHEREUM_DYM_CONTRACT} ===\n`);

  // Setup chain metadata
  const chainMetadata = {
    sepolia: {
      name: 'sepolia',
      chainId: 11155111,
      domainId: 11155111, // Domain ID is required
      protocol: ProtocolType.Ethereum,
      rpcUrls: [{ http: 'https://rpc.sepolia.org' }],
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

  try {
    // Step 1: Create the warp route reader
    console.log('1. Creating EvmERC20WarpRouteReader...');
    const reader = new EvmERC20WarpRouteReader(multiProvider, chain);

    // Step 2: Derive token type
    console.log('2. Deriving token type...');
    const tokenType = await reader.deriveTokenType(ETHEREUM_DYM_CONTRACT);
    console.log(`   Token type: ${tokenType}`);

    // Step 3: Read router configuration
    console.log('3. Reading router configuration...');
    const routerConfig = await reader.readRouterConfig(ETHEREUM_DYM_CONTRACT);
    console.log('   Router config keys:', Object.keys(routerConfig));

    if (routerConfig.remoteRouters) {
      const remoteRouterDomains = Object.keys(routerConfig.remoteRouters);
      console.log(`   Remote router domains: ${remoteRouterDomains.length > 0 ? remoteRouterDomains.join(', ') : 'NONE'}`);
      if (remoteRouterDomains.length === 0) {
        console.log('   ⚠️  No remote routers found!');
      }
    }

    // Step 4: Check domains directly
    console.log('\n4. Checking domains directly from contract...');
    const router = Router__factory.connect(ETHEREUM_DYM_CONTRACT, provider);
    try {
      const domains = await router.domains();
      console.log(`   Raw domains from contract: ${domains.length > 0 ? domains.map(d => d.toString()).join(', ') : 'NONE'}`);

      if (domains.length === 0) {
        console.log('   ⚠️  No domains registered on the contract!');
        console.log('   This explains why remoteRouters is empty.');
        console.log('   The contract may not have enrolled remote routers yet.');
      } else {
        // Try to read router addresses for each domain
        for (const domain of domains) {
          const routerAddress = await router.routers(domain);
          console.log(`   Domain ${domain} -> Router: ${routerAddress}`);
        }
      }
    } catch (error) {
      console.log(`   Error reading domains: ${error.message}`);
    }

    // Step 5: Check destination gas
    console.log('\n5. Checking destination gas...');
    const tokenRouter = TokenRouter__factory.connect(ETHEREUM_DYM_CONTRACT, provider);
    try {
      const domains = await tokenRouter.domains();
      if (domains.length === 0) {
        console.log('   No domains, so no destination gas configured');
      } else {
        for (const domain of domains) {
          const gas = await tokenRouter.destinationGas(domain);
          console.log(`   Domain ${domain} -> Gas: ${gas.toString()}`);
        }
      }
    } catch (error) {
      console.log(`   Error reading destination gas: ${error.message}`);
    }

    // Step 6: Full warp route config
    console.log('\n6. Deriving full warp route config...');
    const fullConfig = await reader.deriveWarpRouteConfig(ETHEREUM_DYM_CONTRACT);
    console.log('   Full config keys:', Object.keys(fullConfig));
    console.log('   Type:', fullConfig.type);
    console.log('   Remote routers:', JSON.stringify(fullConfig.remoteRouters, null, 2));
    console.log('   Destination gas:', JSON.stringify(fullConfig.destinationGas, null, 2));

  } catch (error) {
    console.error('\n❌ Error during debugging:', error);
    if (error.stack) {
      console.error('Stack trace:', error.stack);
    }
  }
}

// Run the debug script
debugWarpRouteCLI().catch(error => {
  console.error('Fatal error:', error);
  process.exit(1);
});