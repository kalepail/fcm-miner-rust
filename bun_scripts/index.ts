import { SorobanRpc, xdr, Address, nativeToScVal, scValToNative } from '@stellar/stellar-sdk';
import type { Subprocess } from 'bun';
import { $ } from 'bun';

const MINER = 'GBDVX4VELCDSQ54KQJYTNHXAHFLBCA77ZY2USQBM4CSHTTV7DME7KALE';
const CONTRACT_ID = 'CC5TSJ3E26YUYGYQKOBNJQLPX4XMUHUY7Q26JX53CJ2YUIZB5HVXXRV6';
const MESSAGE = `KALE`;

const rpc = new SorobanRpc.Server(Bun.env.RPC_URL!);

interface Data {
    hash: string
    current: bigint,
    difficulty: number,
    fcm: string,
    finder: string,
    is_nuked: boolean,
}

async function getContractData() {
    try {
        let result: Data = await rpc.getContractData(
            CONTRACT_ID,
            xdr.ScVal.scvLedgerKeyContractInstance()
        ).then(({ val }) =>
            val.contractData()
                .val()
                .instance()
                .storage()?.[0].val()!
        ).then((scval) => scValToNative(scval));

        let blockLedgerKey = xdr.LedgerKey.contractData(
            new xdr.LedgerKeyContractData({
                contract: new Address(CONTRACT_ID).toScAddress(),
                key: xdr.ScVal.scvVec([
                    xdr.ScVal.scvSymbol("Block"),
                    nativeToScVal(Number(result.current), { type: "u64" })
                ]),
                durability: xdr.ContractDataDurability.persistent(),
            })
        );

        result.hash = await rpc.getLedgerEntries(blockLedgerKey)
            .then(({ entries }) => entries?.[0].val.contractData().val())
            .then((scval) => scValToNative(scval)?.hash?.toString('hex'))

        return result;
    } catch (err) {
        console.error(err);
    }
}

async function bootProc(result: Data) {
    console.log('Proc booted');

    const proc = Bun.spawn([
        '../target/release/fcm-miner-rust', 
        '--index', (result.current + BigInt(1)).toString(),
        '--prev-hash', result.hash,
        '--target-zeros', result.difficulty.toString()
    ], { stdout: 'pipe' })

    const reader = proc.stdout.getReader();

    async function readStream() {
        while (true) {
            const { done, value } = await reader.read();

            try {
                await Bun.write(Bun.stdout, value!);
            } catch(err) {
                console.error(err);
            }

            try {
                let [nonce, hash] = JSON.parse(Buffer.from(value!).toString('utf-8'))

                try {
                    await $`stellar contract invoke --id ${CONTRACT_ID} \
                        --network vc \
                        --source live \
                        -- mine \
                        --nonce ${nonce} \
                        --hash ${hash} \
                        --message ${MESSAGE} \
                        --miner ${MINER}`
                } catch (err) {
                    console.error(err)
                    clearInterval(interval)
                }

                break;
            } catch {}

            if (done) break;
        }
    }

    readStream();

    return proc
}

let prev_result: Data | undefined
let proc: Subprocess | undefined

const interval = setInterval(async () => {
    let result = await getContractData()

    if (result && result.hash !== prev_result?.hash) {
        if (proc) {
            proc.kill()
        }

        console.log(result.current + BigInt(1), result.difficulty, result.hash);
        prev_result = result
    }

    if (result && (!proc || proc.killed)) {
        proc = await bootProc(result)
    }
}, 1000)