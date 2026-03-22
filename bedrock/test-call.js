#!/usr/bin/env node
/**
 * Direct ContractCall test — bypasses bedrock CLI to debug transaction encoding.
 * Usage: node test-call.js
 */
const MODULES = '/Users/paul/.cache/bedrock/modules/node_modules';
const { Wallet } = require(MODULES + '/@transia/xrpl');
const { encode, encodeForSigning } = require(MODULES + '/@transia/ripple-binary-codec');

const POOL = 'rPTEa9QzckBvCRunvc9P4myuXU2tZPgwoV';
const SEED  = 'snuFTAkDRWxMZdE1APubjQzeS7EcS';
const RPC   = 'http://localhost:5005';

// Manager bytes: rGvHv1fAExeTZQA87s4sHWodKUp1nwupgn
// mgr_lo=10054513021433192110, mgr_mid=17601618719508414398, mgr_hi=3165818125
const MGR_LO  = BigInt('10054513021433192110');
const MGR_MID = BigInt('17601618719508414398');
const MGR_HI  = 3165818125;

function toHex64(n) { return n.toString(16).padStart(16,'0').toUpperCase(); }
function toHex32(n) { return n.toString(16).padStart(8,'0').toUpperCase(); }
function toHex16(n) { return n.toString(16).padStart(4,'0').toUpperCase(); }

async function httpRPC(method, params) {
  const https = require('http');
  return new Promise((resolve, reject) => {
    const body = JSON.stringify({ method, params: [params] });
    const req = https.request({
      hostname: 'localhost', port: 5005, path: '/', method: 'POST',
      headers: { 'Content-Type': 'application/json', 'Content-Length': Buffer.byteLength(body) }
    }, res => {
      let data = '';
      res.on('data', d => data += d);
      res.on('end', () => {
        try { resolve(JSON.parse(data).result || JSON.parse(data)); }
        catch(e) { reject(e); }
      });
    });
    req.on('error', reject);
    req.write(body);
    req.end();
  });
}

async function main() {
  const wallet = Wallet.fromSeed(SEED, { algorithm: 'secp256k1' });
  console.log('Wallet:', wallet.address, 'pubkey:', wallet.publicKey);

  const accountInfo = await httpRPC('account_info', { account: wallet.address, ledger_index: 'current' });
  console.log('AccountInfo result:', JSON.stringify(accountInfo).substring(0, 200));
  const seq = accountInfo.account_data.Sequence;
  console.log('Sequence:', seq);

  const POOL = 'r9QjcWXtQKwnAdM74XukXBPuaqfmtYgi8c';
  // initialize_pool(initial_tick: UINT32, fee_bps: UINT16, protocol_fee_share_bps: UINT16)
  // initial_tick=0 (price=1.0), fee_bps=30 (0.3%), protocol_fee_share_bps=0
  const funcNameHex = Buffer.from('initialize_pool').toString('hex').toUpperCase();

  const tx = {
    TransactionType: 'ContractCall',
    Account: wallet.address,
    ContractAccount: POOL,
    FunctionName: funcNameHex,
    Parameters: [
      { Parameter: { ParameterFlag: 0, ParameterValue: { type: 'UINT32', value: 0 } } },
      { Parameter: { ParameterFlag: 0, ParameterValue: { type: 'UINT16', value: 30 } } },
      { Parameter: { ParameterFlag: 0, ParameterValue: { type: 'UINT16', value: 0 } } },
    ],
    ComputationAllowance: 1000000,
    Fee: '1000000',
    Sequence: seq,
    SigningPubKey: wallet.publicKey,
    NetworkID: 63456,
  };

  console.log('\nTransaction (pre-sign):');
  console.log(JSON.stringify(tx, null, 2));

  try {
    const signingBytes = encodeForSigning(tx);
    console.log('\nSigning bytes (hex):', signingBytes.substring(0, 80) + '...');
  } catch(e) {
    console.error('encodeForSigning error:', e.message);
    return;
  }

  let signed;
  try {
    signed = wallet.sign(tx);
    console.log('\nSigned TX hash:', signed.hash);
    console.log('TX blob (first 100 chars):', signed.tx_blob.substring(0, 100));
  } catch(e) {
    console.error('Sign error:', e.message);
    return;
  }

  const result = await httpRPC('submit', { tx_blob: signed.tx_blob });
  console.log('\nSubmit result:');
  console.log(JSON.stringify(result, (k,v) => typeof v === 'bigint' ? v.toString() : v, 2));
}

main().catch(e => { console.error('Fatal:', e); process.exit(1); });
