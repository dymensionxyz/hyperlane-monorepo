console.log('Hello, world!');

import { HyperlaneModuleClient, SigningHyperlaneModuleClient } from "@hyperlane-xyz/cosmos-sdk";
import { DirectSecp256k1Wallet } from '@cosmjs/proto-signing';

// const HUB_RPC_URL = "https://rpc-endpoint:26657";
// const HUB_RPC_URL = "tcp://0.0.0.0:36657";
// const HUB_RPC_URL = "localhost:36657";
const HUB_RPC_URL = "http://localhost:36657";


// using hyperlane queries without needing signers
const client = await HyperlaneModuleClient.connect(
    HUB_RPC_URL
);

const mailboxes = await client.query.core.Mailboxes();
const bridgedSupply = await client.query.warp.BridgedSupply({ id: "token-id" });

// performing hyperlane transactions
const wallet = await DirectSecp256k1Wallet.fromKey(PRIV_KEY);

const signer = await SigningHyperlaneModuleClient.connectWithSigner(
    HUB_RPC_URL,
    wallet,
);

const { response: mailbox } = await signer.createMailbox({
    owner: '...',
    local_domain: '...',
    default_ism: '...',
    default_hook: '...',
    required_hook: '...',
});

const mailboxId = mailbox.id;

await signer.remoteTransfer({
    sender: '...',
    token_id: '...',
    destination_domain: '...',
    recipient: '...',
    amount: '...',
    ...
});

// sign and broadcast custom messages
await signer.signAndBroadcast(signer.getAccounts()[0], [txs...]);