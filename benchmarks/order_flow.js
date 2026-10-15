import http from 'k6/http';
import exec from 'k6/execution';
import { Counter, Rate } from 'k6/metrics';

// Used ai help to write this script 

// This script acts like many clients are placing order
// Before the timed test, it creates a seller and buyer for each selected market, signs them in, 
// deposits the seller’s asset, and onramps the buyer’s USDC. Then, for each scheduled iteration, 
// it sends a limit sell order followed by a matching limit buy order. 
// It expects the sell to be New and the buy to be Filled.

// PAIRS_PER_SECOND controls how many of those sell-and-buy iterations k6 tries to start each second across all selected markets. 
// The script records accepted orders and filled-buy responses; k6 also reports HTTP latency, failures, and iterations it couldn’t start.

const baseUrl = __ENV.BASE_URL || 'http://127.0.0.1:8080';
const pairsPerSecond = Number(__ENV.PAIRS_PER_SECOND || 10);
const durationSeconds = Number(__ENV.DURATION_SECONDS || 30);
const markets = (__ENV.MARKETS || 'SOL_USDC').split(',').map((market) => market.trim());
const price = '100';
const quantity = '1';

if (!Number.isInteger(pairsPerSecond) || pairsPerSecond < 1 ||
  !Number.isInteger(durationSeconds) || durationSeconds < 1 ||
  markets.some((market) => !['SOL_USDC', 'BTC_USDC', 'ETH_USDC', 'HYPE_USDC'].includes(market))) {
  throw new Error('Use positive PAIRS_PER_SECOND/DURATION_SECONDS and supported MARKETS');
}

const ordersPlaced = new Counter('orders_placed');
const pairsFilled = new Counter('pairs_filled');
const orderAccepted = new Rate('order_accepted');

export const options = {
  scenarios: {
    matched_orders: {
      executor: 'constant-arrival-rate',
      rate: pairsPerSecond, // One iteration sends one sell and one buy.
      timeUnit: '1s',
      duration: `${durationSeconds}s`,
      preAllocatedVUs: Number(__ENV.VUS || 20),
      maxVUs: Number(__ENV.MAX_VUS || 200),
    },
  },
  summaryTrendStats: ['avg', 'med', 'p(95)', 'p(99)', 'max'],
  thresholds: {
    order_accepted: ['rate>0.99'],
    dropped_iterations: ['count==0'],
    'http_req_duration{name:order}': ['p(95)<2000'],
  },
};

function postJson(path, body, token, name) {
  const headers = { 'Content-Type': 'application/json' };
  if (token) headers.Authorization = token; // This API expects the raw JWT.
  return http.post(`${baseUrl}${path}`, JSON.stringify(body), {
    headers,
    tags: { name },
  });
}

function createUser(username) {
  const credentials = { username, password: 'benchmark-only-password' };
  const signup = postJson('/signup', credentials, null, 'setup');
  if (signup.status !== 200) throw new Error(`Signup failed: HTTP ${signup.status}`);

  const signin = postJson('/signin', credentials, null, 'setup');
  if (signin.status !== 200 || !signin.json('token')) {
    throw new Error(`Signin failed: HTTP ${signin.status}`);
  }
  return signin.json('token');
}

export function setup() {
  const runId = `bench_${Date.now()}_${Math.floor(Math.random() * 1e6)}`;
  const fundedPairs = Math.ceil(pairsPerSecond * durationSeconds * 1.2);
  const accounts = markets.map((market, index) => {
    const seller = createUser(`${runId}_${index}_seller`);
    const buyer = createUser(`${runId}_${index}_buyer`);
    const asset = market.split('_')[0];

    const deposit = postJson('/balance/deposit', {
      asset,
      quantity: String(fundedPairs),
    }, seller, 'setup');
    const onramp = postJson('/balance/onramp', {
      amount: String(fundedPairs * Number(price)),
    }, buyer, 'setup');
    if (deposit.status !== 200 || !deposit.json('DepositResponse') ||
      onramp.status !== 200 || !onramp.json('OnrampResponse')) {
      throw new Error(`Funding failed for ${market}`);
    }

    return { market, seller, buyer };
  });

  return { accounts };
}

function placeOrder(token, market, side) {
  const response = postJson('/order', {
    symbol: market,
    order_type: 'Limit',
    order_side: side,
    price,
    quantity,
  }, token, 'order');

  let placed = null;
  if (response.status === 200) {
    try {
      placed = response.json('OrderPlaced');
    } catch (_) {
      // A malformed response counts as a failed order.
    }
  }
  const accepted = !!placed && placed.order_status === (side === 'Sell' ? 'New' : 'Filled');
  orderAccepted.add(accepted);
  if (accepted) ordersPlaced.add(1);
  return accepted;
}

export default function(data) {
  const account = data.accounts[exec.scenario.iterationInTest % data.accounts.length];
  if (!placeOrder(account.seller, account.market, 'Sell')) return;
  if (placeOrder(account.buyer, account.market, 'Buy')) pairsFilled.add(1);
}
