import { Address, xdr } from '@stellar/stellar-sdk';

let index = xdr.ScVal.scvU64(xdr.Uint64.MAX_VALUE).toXDR()
let message = xdr.ScVal.scvString('KALE').toXDR()
let prev_hash = xdr.ScVal.scvBytes(Buffer.alloc(32).fill(255)).toXDR()
let nonce = xdr.ScVal.scvU64(xdr.Uint64.fromString('1234')).toXDR()
let miner = Address.fromString('GBDVX4VELCDSQ54KQJYTNHXAHFLBCA77ZY2USQBM4CSHTTV7DME7KALE').toScVal().toXDR()

console.log(`
    ${index.toJSON().data}
    ${message.toJSON().data}
    ${prev_hash.toJSON().data}
    ${nonce.toJSON().data}
    ${miner.toJSON().data}
`);

// index = [0, 0, 0, 5]
// message = [0, 0, 0, 14, 0, 0, 0, 4]
// prev_hash = [0, 0, 0, 13, 0, 0, 0, 32]
// nonce = [0, 0, 0, 5]
// miner = [0, 0, 0, 18, 0, 0, 0, 0, 0, 0, 0, 0]