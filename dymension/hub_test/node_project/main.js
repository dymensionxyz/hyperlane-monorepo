import {
  DirectSecp256k1HdWallet,
  DirectSecp256k1Wallet,
} from '@cosmjs/proto-signing';
import { GasPrice } from '@cosmjs/stargate';

import {
  HyperlaneModuleClient,
  SigningHyperlaneModuleClient,
} from '@hyperlane-xyz/cosmos-sdk';

console.log('Hello, world!');

//////////////////////////////
// STEP: CONFIGURE CLIENTS

// const HUB_RPC_URL = "https://rpc-endpoint:26657";
// const HUB_RPC_URL = "tcp://0.0.0.0:36657";
// const HUB_RPC_URL = "localhost:36657";
const HUB_RPC_URL = 'http://localhost:36657';

// hub keys export hub-user --unsafe --unarmored-hex
const HUB_USER_KEY =
  '9b0cf6ae685ab2906df05a286154ba1309414af98cb87a95fc1719316a2dcc13';
// dym1clquwldahwyu2l2595fra6je8grq82y4mxvfl6
const HUB_USER_MEM =
  'unable wall same plunge man guard above valid despair census alcohol coin such tunnel protect coffee chest evoke license intact angle regular turkey escape';

const HUB_PREF = 'dym';

// const wallet = await DirectSecp256k1Wallet.fromKey(Buffer.from(HUB_USER_KEY, 'hex'), HUB_PREF);
const wallet = await DirectSecp256k1HdWallet.fromMnemonic(HUB_USER_MEM, {
  prefix: HUB_PREF,
});

const signer = await SigningHyperlaneModuleClient.connectWithSigner(
  HUB_RPC_URL,
  wallet,
  {
    gasPrice: GasPrice.fromString('0.2udym'), // TODO: check
  },
);

const hubDomain = 0;
const ethDomain = 1;

//////////////////////////////
// STEP: DEPLOY HYPERLANE ENTITIES TO HUB

///////////////
// Part 1. Core
// Order is: 1. Noop ISM, 2. Mailbox, 3. Noop hook, 4. Configure mailbox with noop hook
// TODO: think I can do 1. noop ism, 2. noop hook, 3. mailbox

await signer.createNoopIsm({});
const { isms } = await signer.query.interchainSecurity.DecodedIsms({});
const ismId = isms[0].id;

// TODO: think I can do 1. noop ism, 2. noop hook, 3. mailbox
await signer.createNoopHook({});
const { noopHooks } = await signer.query.core.NoopHooks({});
const noopHookId = noopHooks[0].id;

const txResponse = await signer.createMailbox({
  local_domain: hubDomain,
  default_ism: ismId,
  default_hook: noopHookId,
  required_hook: noopHookId,
});

const mailboxes = await signer.query.core.Mailboxes({});
const mailboxId = mailboxes.mailboxes[0].id;
// TODO: gas config needed?

///////////////
// Part 2. Warp
// Order 1. Create collateral (memo), 2. Enroll remote router.

const denom = 'adym';

await signer.createCollateralToken({
  origin_mailbox: mailbox.id,
  origin_denom: denom,
});

const tokens = await signer.query.warp.Tokens({});
const tokenId = tokens.tokens[0].id;

const gas = '10000';

const routers = await signer.enrollRemoteRouter({
  token_id: token.id,
  remote_router: {
    receiver_domain: mailbox.local_domain,
    receiver_contract: mailbox.id,
    gas,
  },
});

const remoteRouter = remoteRouters.remote_routers[0];
