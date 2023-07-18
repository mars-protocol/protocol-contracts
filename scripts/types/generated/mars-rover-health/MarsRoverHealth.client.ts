// @ts-nocheck
/**
 * This file was automatically generated by @cosmwasm/ts-codegen@0.33.0.
 * DO NOT MODIFY IT BY HAND. Instead, modify the source JSONSchema file,
 * and run the @cosmwasm/ts-codegen generate command to regenerate this file.
 */

import { CosmWasmClient, SigningCosmWasmClient, ExecuteResult } from '@cosmjs/cosmwasm-stargate'
import { Coin, StdFee } from '@cosmjs/amino'
import {
  InstantiateMsg,
  ExecuteMsg,
  OwnerUpdate,
  QueryMsg,
  ActionKind,
  AccountKind,
  ConfigResponse,
  OwnerResponse,
  HealthState,
  Decimal,
  Uint128,
  HealthValuesResponse,
} from './MarsRoverHealth.types'
export interface MarsRoverHealthReadOnlyInterface {
  contractAddress: string
  healthValues: ({
    accountId,
    action,
    kind,
  }: {
    accountId: string
    action: ActionKind
    kind: AccountKind
  }) => Promise<HealthValuesResponse>
  healthState: ({
    accountId,
    action,
    kind,
  }: {
    accountId: string
    action: ActionKind
    kind: AccountKind
  }) => Promise<HealthState>
  config: () => Promise<ConfigResponse>
}
export class MarsRoverHealthQueryClient implements MarsRoverHealthReadOnlyInterface {
  client: CosmWasmClient
  contractAddress: string

  constructor(client: CosmWasmClient, contractAddress: string) {
    this.client = client
    this.contractAddress = contractAddress
    this.healthValues = this.healthValues.bind(this)
    this.healthState = this.healthState.bind(this)
    this.config = this.config.bind(this)
  }

  healthValues = async ({
    accountId,
    action,
    kind,
  }: {
    accountId: string
    action: ActionKind
    kind: AccountKind
  }): Promise<HealthValuesResponse> => {
    return this.client.queryContractSmart(this.contractAddress, {
      health_values: {
        account_id: accountId,
        action,
        kind,
      },
    })
  }
  healthState = async ({
    accountId,
    action,
    kind,
  }: {
    accountId: string
    action: ActionKind
    kind: AccountKind
  }): Promise<HealthState> => {
    return this.client.queryContractSmart(this.contractAddress, {
      health_state: {
        account_id: accountId,
        action,
        kind,
      },
    })
  }
  config = async (): Promise<ConfigResponse> => {
    return this.client.queryContractSmart(this.contractAddress, {
      config: {},
    })
  }
}
export interface MarsRoverHealthInterface extends MarsRoverHealthReadOnlyInterface {
  contractAddress: string
  sender: string
  updateOwner: (
    ownerUpdate: OwnerUpdate,
    fee?: number | StdFee | 'auto',
    memo?: string,
    _funds?: Coin[],
  ) => Promise<ExecuteResult>
  updateConfig: (
    {
      creditManager,
    }: {
      creditManager: string
    },
    fee?: number | StdFee | 'auto',
    memo?: string,
    _funds?: Coin[],
  ) => Promise<ExecuteResult>
}
export class MarsRoverHealthClient
  extends MarsRoverHealthQueryClient
  implements MarsRoverHealthInterface
{
  client: SigningCosmWasmClient
  sender: string
  contractAddress: string

  constructor(client: SigningCosmWasmClient, sender: string, contractAddress: string) {
    super(client, contractAddress)
    this.client = client
    this.sender = sender
    this.contractAddress = contractAddress
    this.updateOwner = this.updateOwner.bind(this)
    this.updateConfig = this.updateConfig.bind(this)
  }

  updateOwner = async (
    ownerUpdate: OwnerUpdate,
    fee: number | StdFee | 'auto' = 'auto',
    memo?: string,
    _funds?: Coin[],
  ): Promise<ExecuteResult> => {
    return await this.client.execute(
      this.sender,
      this.contractAddress,
      {
        update_owner: ownerUpdate,
      },
      fee,
      memo,
      _funds,
    )
  }
  updateConfig = async (
    {
      creditManager,
    }: {
      creditManager: string
    },
    fee: number | StdFee | 'auto' = 'auto',
    memo?: string,
    _funds?: Coin[],
  ): Promise<ExecuteResult> => {
    return await this.client.execute(
      this.sender,
      this.contractAddress,
      {
        update_config: {
          credit_manager: creditManager,
        },
      },
      fee,
      memo,
      _funds,
    )
  }
}
