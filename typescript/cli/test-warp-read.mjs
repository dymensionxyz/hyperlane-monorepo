#!/usr/bin/env node
// Simple test to reproduce the exact CLI behavior
import { execSync } from 'child_process';

const ETHEREUM_DYM_CONTRACT = process.env.ETHEREUM_DYM_CONTRACT || '0x626991CA73756C1c704b3B1D567634FC97936CB6';

console.log(`\n=== Testing hyperlane warp read ===\n`);
console.log(`Contract: ${ETHEREUM_DYM_CONTRACT}`);

// First, let's see what the actual CLI command outputs
const command = `hyperlane warp read --chain sepolia --address ${ETHEREUM_DYM_CONTRACT} --registry https://github.com/hyperlane-xyz/hyperlane-registry --verbosity debug`;

console.log(`\nRunning: ${command}\n`);
console.log('=' .repeat(80));

try {
  const output = execSync(command, {
    encoding: 'utf8',
    env: { ...process.env },
    timeout: 60000
  });
  console.log(output);
} catch (error) {
  console.error('Command failed:', error.message);
  if (error.stdout) console.log('STDOUT:', error.stdout);
  if (error.stderr) console.log('STDERR:', error.stderr);
}