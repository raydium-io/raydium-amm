// @ts-nocheck
import * as B from "@native-to-anchor/buffer-layout";
import { AccountsCoder, Idl } from "@project-serum/anchor";
import { IdlTypeDef } from "@project-serum/anchor/dist/cjs/idl";

export class RaydiumAmmAccountsCoder<A extends string = string>
  implements AccountsCoder
{
  constructor(_idl: Idl) {}

  public async encode<T = any>(accountName: A, account: T): Promise<Buffer> {
    switch (accountName) {
      case "targetOrders": {
    const buffer = Buffer.alloc(2208); 
    const len = TARGET_ORDERS_LAYOUT.encode(account, buffer);
    return buffer.slice(0, len);
}case "fees": {
    const buffer = Buffer.alloc(64); 
    const len = FEES_LAYOUT.encode(account, buffer);
    return buffer.slice(0, len);
}case "ammInfo": {
    const buffer = Buffer.alloc(752); 
    const len = AMM_INFO_LAYOUT.encode(account, buffer);
    return buffer.slice(0, len);
}case "ammConfig": {
    const buffer = Buffer.alloc(544); 
    const len = AMM_CONFIG_LAYOUT.encode(account, buffer);
    return buffer.slice(0, len);
}
      default: {
        throw new Error(`Invalid account name: ${accountName}`);
      }
    }
  }

  public decode<T = any>(accountName: A, ix: Buffer): T {
    return this.decodeUnchecked(accountName, ix);
  }

  public decodeUnchecked<T = any>(accountName: A, ix: Buffer): T {
    switch (accountName) {
      case "targetOrders": {
    return decodeTargetOrdersAccount(ix);
}case "fees": {
    return decodeFeesAccount(ix);
}case "ammInfo": {
    return decodeAmmInfoAccount(ix);
}case "ammConfig": {
    return decodeAmmConfigAccount(ix);
}
      default: {
        throw new Error(`Invalid account name: ${accountName}`);
      }
    }
  }

  public memcmp(
    accountName: A,
    _appendData?: Buffer
  ): { dataSize?: number, offset?: number, bytes?: string } {
    switch (accountName) {
      case "targetOrders": {
    return {
        dataSize: 2208,
    };
    
}case "fees": {
    return {
        dataSize: 64,
    };
    
}case "ammInfo": {
    return {
        dataSize: 752,
    };
    
}case "ammConfig": {
    return {
        dataSize: 544,
    };
    
}
      default: {
        throw new Error(`Invalid account name: ${accountName}`);
      }
    }
  }

  public size(idlAccount: IdlTypeDef): number {
    switch (idlAccount.name) {
      case "targetOrders": {
    return 2208 ;
}case "fees": {
    return 64 ;
}case "ammInfo": {
    return 752 ;
}case "ammConfig": {
    return 544 ;
}
      default: {
        throw new Error(`Invalid account name: ${idlAccount.name}`);
      }
    }
  }
}

function decodeTargetOrdersAccount<T = any>(ix: Buffer): T {
    return TARGET_ORDERS_LAYOUT.decode(ix) as T;
}
function decodeFeesAccount<T = any>(ix: Buffer): T {
    return FEES_LAYOUT.decode(ix) as T;
}
function decodeAmmInfoAccount<T = any>(ix: Buffer): T {
    return AMM_INFO_LAYOUT.decode(ix) as T;
}
function decodeAmmConfigAccount<T = any>(ix: Buffer): T {
    return AMM_CONFIG_LAYOUT.decode(ix) as T;
}


const TARGET_ORDERS_LAYOUT: any = B.struct([B.seq(B.u64(), 4, "owner"),B.seq(B.struct([B.u64("price"),B.u64("vol"),], ), 50, "buyOrders"),B.seq(B.u64(), 8, "padding1"),B.u128("targetX"),B.u128("targetY"),B.u128("planXBuy"),B.u128("planYBuy"),B.u128("planXSell"),B.u128("planYSell"),B.u128("placedX"),B.u128("placedY"),B.u128("calcPnlX"),B.u128("calcPnlY"),B.seq(B.struct([B.u64("price"),B.u64("vol"),], ), 50, "sellOrders"),B.seq(B.u64(), 6, "padding2"),B.seq(B.u64(), 10, "replaceBuyClientId"),B.seq(B.u64(), 10, "replaceSellClientId"),B.u64("lastOrderNumerator"),B.u64("lastOrderDenominator"),B.u64("planOrdersCur"),B.u64("placeOrdersCur"),B.u64("validBuyOrderNum"),B.u64("validSellOrderNum"),B.seq(B.u64(), 10, "padding3"),B.u128("freeSlotBits"),]);

const FEES_LAYOUT: any = B.struct([B.u64("minSeparateNumerator"),B.u64("minSeparateDenominator"),B.u64("tradeFeeNumerator"),B.u64("tradeFeeDenominator"),B.u64("pnlNumerator"),B.u64("pnlDenominator"),B.u64("swapFeeNumerator"),B.u64("swapFeeDenominator"),]);

const AMM_INFO_LAYOUT: any = B.struct([B.u64("status"),B.u64("nonce"),B.u64("orderNum"),B.u64("depth"),B.u64("coinDecimals"),B.u64("pcDecimals"),B.u64("state"),B.u64("resetFlag"),B.u64("minSize"),B.u64("volMaxCutRatio"),B.u64("amountWave"),B.u64("coinLotSize"),B.u64("pcLotSize"),B.u64("minPriceMultiplier"),B.u64("maxPriceMultiplier"),B.u64("sysDecimalValue"),B.struct([B.u64("minSeparateNumerator"),B.u64("minSeparateDenominator"),B.u64("tradeFeeNumerator"),B.u64("tradeFeeDenominator"),B.u64("pnlNumerator"),B.u64("pnlDenominator"),B.u64("swapFeeNumerator"),B.u64("swapFeeDenominator"),], "fees"),B.struct([B.u64("needTakePnlCoin"),B.u64("needTakePnlPc"),B.u64("totalPnlPc"),B.u64("totalPnlCoin"),B.u64("poolOpenTime"),B.seq(B.u64(), 2, "padding"),B.u64("orderbookToInitTime"),B.u128("swapCoinInAmount"),B.u128("swapPcOutAmount"),B.u64("swapAccPcFee"),B.u128("swapPcInAmount"),B.u128("swapCoinOutAmount"),B.u64("swapAccCoinFee"),], "stateData"),B.publicKey("coinVault"),B.publicKey("pcVault"),B.publicKey("coinVaultMint"),B.publicKey("pcVaultMint"),B.publicKey("lpMint"),B.publicKey("openOrders"),B.publicKey("market"),B.publicKey("marketProgram"),B.publicKey("targetOrders"),B.seq(B.u64(), 8, "padding1"),B.publicKey("ammOwner"),B.u64("lpAmount"),B.u64("clientOrderId"),B.u64("recentEpoch"),B.u64("padding2"),]);

const AMM_CONFIG_LAYOUT: any = B.struct([B.publicKey("pnlOwner"),B.publicKey("cancelOwner"),B.seq(B.u64(), 28, "pending1"),B.seq(B.u64(), 31, "pending2"),B.u64("createPoolFee"),]);

