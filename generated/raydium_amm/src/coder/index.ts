import { Idl, Coder } from "@project-serum/anchor"

import { RaydiumAmmAccountsCoder } from "./accounts";
import { RaydiumAmmEventsCoder } from "./events";
import { RaydiumAmmInstructionCoder } from "./instructions";
import { RaydiumAmmStateCoder } from "./state";
import { RaydiumAmmTypesCoder } from "./types";

/**
 * Coder for RaydiumAmm
 */
export class RaydiumAmmCoder implements Coder {
  readonly accounts: RaydiumAmmAccountsCoder;
  readonly events: RaydiumAmmEventsCoder;
  readonly instruction: RaydiumAmmInstructionCoder;
  readonly state: RaydiumAmmStateCoder;
  readonly types: RaydiumAmmTypesCoder;

  constructor(idl: Idl) {
    this.accounts = new RaydiumAmmAccountsCoder(idl);
    this.events = new RaydiumAmmEventsCoder(idl);
    this.instruction = new RaydiumAmmInstructionCoder(idl);
    this.state = new RaydiumAmmStateCoder(idl);
    this.types = new RaydiumAmmTypesCoder(idl);
  }
}
