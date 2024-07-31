// @ts-nocheck
import * as B from "@native-to-anchor/buffer-layout";
import { Idl, InstructionCoder } from "@project-serum/anchor";

export class RaydiumAmmInstructionCoder implements InstructionCoder {
  constructor(_idl: Idl) {}

  encode(ixName: string, ix: any): Buffer {
    switch (ixName) {
      case "initialize": {return encodeInitialize(ix);}
case "initialize2": {return encodeInitialize2(ix);}
case "monitorStep": {return encodeMonitorStep(ix);}
case "deposit": {return encodeDeposit(ix);}
case "withdraw": {return encodeWithdraw(ix);}
case "migrateToOpenBook": {return encodeMigrateToOpenBook(ix);}
case "setParams": {return encodeSetParams(ix);}
case "withdrawPnl": {return encodeWithdrawPnl(ix);}
case "withdrawSrm": {return encodeWithdrawSrm(ix);}
case "swapBaseIn": {return encodeSwapBaseIn(ix);}
case "preInitialize": {return encodePreInitialize(ix);}
case "swapBaseOut": {return encodeSwapBaseOut(ix);}
case "simulateInfo": {return encodeSimulateInfo(ix);}
case "adminCancelOrders": {return encodeAdminCancelOrders(ix);}
case "createConfigAccount": {return encodeCreateConfigAccount(ix);}
case "updateConfigAccount": {return encodeUpdateConfigAccount(ix);}

      default: {
        throw new Error(`Invalid instruction: ${ixName}`);
      }
    }
  }

  encodeState(_ixName: string, _ix: any): Buffer {
    throw new Error("RaydiumAmm does not have state");
  }
}

function encodeInitialize({nonce,openTime,}: any): Buffer {return encodeData({initialize: {nonce,openTime,}}, 1+ 1+ 8);}

function encodeInitialize2({nonce,openTime,initPcAmount,initCoinAmount,}: any): Buffer {return encodeData({initialize2: {nonce,openTime,initPcAmount,initCoinAmount,}}, 1+ 1+ 8+ 8+ 8);}

function encodeMonitorStep({planOrderLimit,placeOrderLimit,cancelOrderLimit,}: any): Buffer {return encodeData({monitorStep: {planOrderLimit,placeOrderLimit,cancelOrderLimit,}}, 1+ 2+ 2+ 2);}

function encodeDeposit({maxCoinAmount,maxPcAmount,baseSide,}: any): Buffer {return encodeData({deposit: {maxCoinAmount,maxPcAmount,baseSide,}}, 1+ 8+ 8+ 8);}

function encodeWithdraw({amount,}: any): Buffer {return encodeData({withdraw: {amount,}}, 1+ 8);}

function encodeMigrateToOpenBook({}: any): Buffer {return encodeData({migrateToOpenBook: {}}, 1);}

function encodeSetParams({param,value,newPubkey,fees,lastOrderDistance,}: any): Buffer {return encodeData({setParams: {param,value,newPubkey,fees,lastOrderDistance,}}, 1+ 1+1 + (value === null ? 0 : 8)+1 + (newPubkey === null ? 0 : 32)+1 + (fees === null ? 0 : 64)+1 + (lastOrderDistance === null ? 0 : 16));}

function encodeWithdrawPnl({}: any): Buffer {return encodeData({withdrawPnl: {}}, 1);}

function encodeWithdrawSrm({amount,}: any): Buffer {return encodeData({withdrawSrm: {amount,}}, 1+ 8);}

function encodeSwapBaseIn({amountIn,minimumAmountOut,}: any): Buffer {return encodeData({swapBaseIn: {amountIn,minimumAmountOut,}}, 1+ 8+ 8);}

function encodePreInitialize({nonce,}: any): Buffer {return encodeData({preInitialize: {nonce,}}, 1+ 1);}

function encodeSwapBaseOut({maxAmountIn,amountOut,}: any): Buffer {return encodeData({swapBaseOut: {maxAmountIn,amountOut,}}, 1+ 8+ 8);}

function encodeSimulateInfo({param,swapBaseInValue,swapBaseOutValue,}: any): Buffer {return encodeData({simulateInfo: {param,swapBaseInValue,swapBaseOutValue,}}, 1+ 1+1 + (swapBaseInValue === null ? 0 : 16)+1 + (swapBaseOutValue === null ? 0 : 16));}

function encodeAdminCancelOrders({limit,}: any): Buffer {return encodeData({adminCancelOrders: {limit,}}, 1+ 2);}

function encodeCreateConfigAccount({}: any): Buffer {return encodeData({createConfigAccount: {}}, 1);}

function encodeUpdateConfigAccount({param,owner,createPoolFee,}: any): Buffer {return encodeData({updateConfigAccount: {param,owner,createPoolFee,}}, 1+ 1+1 + (owner === null ? 0 : 32)+1 + (createPoolFee === null ? 0 : 8));}



const LAYOUT = B.union(B.u8("instruction"));
LAYOUT.addVariant(0, B.struct([B.u8("nonce"),B.u64("openTime"),]), "initialize");LAYOUT.addVariant(1, B.struct([B.u8("nonce"),B.u64("openTime"),B.u64("initPcAmount"),B.u64("initCoinAmount"),]), "initialize2");LAYOUT.addVariant(2, B.struct([B.u16("planOrderLimit"),B.u16("placeOrderLimit"),B.u16("cancelOrderLimit"),]), "monitorStep");LAYOUT.addVariant(3, B.struct([B.u64("maxCoinAmount"),B.u64("maxPcAmount"),B.u64("baseSide"),]), "deposit");LAYOUT.addVariant(4, B.struct([B.u64("amount"),]), "withdraw");LAYOUT.addVariant(5, B.struct([]), "migrateToOpenBook");LAYOUT.addVariant(6, B.struct([B.u8("param"),B.option(B.u64(), "value"),B.option(B.publicKey(), "newPubkey"),B.option(B.struct([B.u64("minSeparateNumerator"),B.u64("minSeparateDenominator"),B.u64("tradeFeeNumerator"),B.u64("tradeFeeDenominator"),B.u64("pnlNumerator"),B.u64("pnlDenominator"),B.u64("swapFeeNumerator"),B.u64("swapFeeDenominator"),], ), "fees"),B.option(B.struct([B.u64("lastOrderNumerator"),B.u64("lastOrderDenominator"),], ), "lastOrderDistance"),]), "setParams");LAYOUT.addVariant(7, B.struct([]), "withdrawPnl");LAYOUT.addVariant(8, B.struct([B.u64("amount"),]), "withdrawSrm");LAYOUT.addVariant(9, B.struct([B.u64("amountIn"),B.u64("minimumAmountOut"),]), "swapBaseIn");LAYOUT.addVariant(10, B.struct([B.u8("nonce"),]), "preInitialize");LAYOUT.addVariant(11, B.struct([B.u64("maxAmountIn"),B.u64("amountOut"),]), "swapBaseOut");LAYOUT.addVariant(12, B.struct([B.u8("param"),B.option(B.struct([B.u64("amountIn"),B.u64("minimumAmountOut"),], ), "swapBaseInValue"),B.option(B.struct([B.u64("maxAmountIn"),B.u64("amountOut"),], ), "swapBaseOutValue"),]), "simulateInfo");LAYOUT.addVariant(13, B.struct([B.u16("limit"),]), "adminCancelOrders");LAYOUT.addVariant(14, B.struct([]), "createConfigAccount");LAYOUT.addVariant(15, B.struct([B.u8("param"),B.option(B.publicKey(), "owner"),B.option(B.u64(), "createPoolFee"),]), "updateConfigAccount");

function encodeData(ix: any, span: number): Buffer {
  const b = Buffer.alloc(span);
  LAYOUT.encode(ix, b);
  return b;
}
