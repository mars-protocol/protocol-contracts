// @ts-nocheck
/**
 * This file was automatically generated by @cosmwasm/ts-codegen@0.33.0.
 * DO NOT MODIFY IT BY HAND. Instead, modify the source JSONSchema file,
 * and run the @cosmwasm/ts-codegen generate command to regenerate this file.
 */

import { UseQueryOptions, useQuery, useMutation, UseMutationOptions } from '@tanstack/react-query'
import { ExecuteResult } from '@cosmjs/cosmwasm-stargate'
import { StdFee } from '@cosmjs/amino'
import {
  InstantiateMsg,
  ExecuteMsg,
  OwnerUpdate,
  Uint128,
  Decimal,
  Addr,
  Empty,
  Coin,
  QueryMsg,
  EstimateExactInSwapResponse,
  OwnerResponse,
  RouteResponseForEmpty,
  ArrayOfRouteResponseForEmpty,
} from './MarsSwapperBase.types'
import { MarsSwapperBaseQueryClient, MarsSwapperBaseClient } from './MarsSwapperBase.client'
export const marsSwapperBaseQueryKeys = {
  contract: [
    {
      contract: 'marsSwapperBase',
    },
  ] as const,
  address: (contractAddress: string | undefined) =>
    [{ ...marsSwapperBaseQueryKeys.contract[0], address: contractAddress }] as const,
  owner: (contractAddress: string | undefined, args?: Record<string, unknown>) =>
    [{ ...marsSwapperBaseQueryKeys.address(contractAddress)[0], method: 'owner', args }] as const,
  route: (contractAddress: string | undefined, args?: Record<string, unknown>) =>
    [{ ...marsSwapperBaseQueryKeys.address(contractAddress)[0], method: 'route', args }] as const,
  routes: (contractAddress: string | undefined, args?: Record<string, unknown>) =>
    [{ ...marsSwapperBaseQueryKeys.address(contractAddress)[0], method: 'routes', args }] as const,
  estimateExactInSwap: (contractAddress: string | undefined, args?: Record<string, unknown>) =>
    [
      {
        ...marsSwapperBaseQueryKeys.address(contractAddress)[0],
        method: 'estimate_exact_in_swap',
        args,
      },
    ] as const,
}
export interface MarsSwapperBaseReactQuery<TResponse, TData = TResponse> {
  client: MarsSwapperBaseQueryClient | undefined
  options?: Omit<
    UseQueryOptions<TResponse, Error, TData>,
    "'queryKey' | 'queryFn' | 'initialData'"
  > & {
    initialData?: undefined
  }
}
export interface MarsSwapperBaseEstimateExactInSwapQuery<TData>
  extends MarsSwapperBaseReactQuery<EstimateExactInSwapResponse, TData> {
  args: {
    coinIn: Coin
    denomOut: string
  }
}
export function useMarsSwapperBaseEstimateExactInSwapQuery<TData = EstimateExactInSwapResponse>({
  client,
  args,
  options,
}: MarsSwapperBaseEstimateExactInSwapQuery<TData>) {
  return useQuery<EstimateExactInSwapResponse, Error, TData>(
    marsSwapperBaseQueryKeys.estimateExactInSwap(client?.contractAddress, args),
    () =>
      client
        ? client.estimateExactInSwap({
            coinIn: args.coinIn,
            denomOut: args.denomOut,
          })
        : Promise.reject(new Error('Invalid client')),
    { ...options, enabled: !!client && (options?.enabled != undefined ? options.enabled : true) },
  )
}
export interface MarsSwapperBaseRoutesQuery<TData>
  extends MarsSwapperBaseReactQuery<ArrayOfRouteResponseForEmpty, TData> {
  args: {
    limit?: number
    startAfter?: string[][]
  }
}
export function useMarsSwapperBaseRoutesQuery<TData = ArrayOfRouteResponseForEmpty>({
  client,
  args,
  options,
}: MarsSwapperBaseRoutesQuery<TData>) {
  return useQuery<ArrayOfRouteResponseForEmpty, Error, TData>(
    marsSwapperBaseQueryKeys.routes(client?.contractAddress, args),
    () =>
      client
        ? client.routes({
            limit: args.limit,
            startAfter: args.startAfter,
          })
        : Promise.reject(new Error('Invalid client')),
    { ...options, enabled: !!client && (options?.enabled != undefined ? options.enabled : true) },
  )
}
export interface MarsSwapperBaseRouteQuery<TData>
  extends MarsSwapperBaseReactQuery<RouteResponseForEmpty, TData> {
  args: {
    denomIn: string
    denomOut: string
  }
}
export function useMarsSwapperBaseRouteQuery<TData = RouteResponseForEmpty>({
  client,
  args,
  options,
}: MarsSwapperBaseRouteQuery<TData>) {
  return useQuery<RouteResponseForEmpty, Error, TData>(
    marsSwapperBaseQueryKeys.route(client?.contractAddress, args),
    () =>
      client
        ? client.route({
            denomIn: args.denomIn,
            denomOut: args.denomOut,
          })
        : Promise.reject(new Error('Invalid client')),
    { ...options, enabled: !!client && (options?.enabled != undefined ? options.enabled : true) },
  )
}
export interface MarsSwapperBaseOwnerQuery<TData>
  extends MarsSwapperBaseReactQuery<OwnerResponse, TData> {}
export function useMarsSwapperBaseOwnerQuery<TData = OwnerResponse>({
  client,
  options,
}: MarsSwapperBaseOwnerQuery<TData>) {
  return useQuery<OwnerResponse, Error, TData>(
    marsSwapperBaseQueryKeys.owner(client?.contractAddress),
    () => (client ? client.owner() : Promise.reject(new Error('Invalid client'))),
    { ...options, enabled: !!client && (options?.enabled != undefined ? options.enabled : true) },
  )
}
export interface MarsSwapperBaseTransferResultMutation {
  client: MarsSwapperBaseClient
  msg: {
    denomIn: string
    denomOut: string
    recipient: Addr
  }
  args?: {
    fee?: number | StdFee | 'auto'
    memo?: string
    funds?: Coin[]
  }
}
export function useMarsSwapperBaseTransferResultMutation(
  options?: Omit<
    UseMutationOptions<ExecuteResult, Error, MarsSwapperBaseTransferResultMutation>,
    'mutationFn'
  >,
) {
  return useMutation<ExecuteResult, Error, MarsSwapperBaseTransferResultMutation>(
    ({ client, msg, args: { fee, memo, funds } = {} }) =>
      client.transferResult(msg, fee, memo, funds),
    options,
  )
}
export interface MarsSwapperBaseSwapExactInMutation {
  client: MarsSwapperBaseClient
  msg: {
    coinIn: Coin
    denomOut: string
    slippage: Decimal
  }
  args?: {
    fee?: number | StdFee | 'auto'
    memo?: string
    funds?: Coin[]
  }
}
export function useMarsSwapperBaseSwapExactInMutation(
  options?: Omit<
    UseMutationOptions<ExecuteResult, Error, MarsSwapperBaseSwapExactInMutation>,
    'mutationFn'
  >,
) {
  return useMutation<ExecuteResult, Error, MarsSwapperBaseSwapExactInMutation>(
    ({ client, msg, args: { fee, memo, funds } = {} }) => client.swapExactIn(msg, fee, memo, funds),
    options,
  )
}
export interface MarsSwapperBaseSetRouteMutation {
  client: MarsSwapperBaseClient
  msg: {
    denomIn: string
    denomOut: string
    route: Empty
  }
  args?: {
    fee?: number | StdFee | 'auto'
    memo?: string
    funds?: Coin[]
  }
}
export function useMarsSwapperBaseSetRouteMutation(
  options?: Omit<
    UseMutationOptions<ExecuteResult, Error, MarsSwapperBaseSetRouteMutation>,
    'mutationFn'
  >,
) {
  return useMutation<ExecuteResult, Error, MarsSwapperBaseSetRouteMutation>(
    ({ client, msg, args: { fee, memo, funds } = {} }) => client.setRoute(msg, fee, memo, funds),
    options,
  )
}
export interface MarsSwapperBaseUpdateOwnerMutation {
  client: MarsSwapperBaseClient
  msg: OwnerUpdate
  args?: {
    fee?: number | StdFee | 'auto'
    memo?: string
    funds?: Coin[]
  }
}
export function useMarsSwapperBaseUpdateOwnerMutation(
  options?: Omit<
    UseMutationOptions<ExecuteResult, Error, MarsSwapperBaseUpdateOwnerMutation>,
    'mutationFn'
  >,
) {
  return useMutation<ExecuteResult, Error, MarsSwapperBaseUpdateOwnerMutation>(
    ({ client, msg, args: { fee, memo, funds } = {} }) => client.updateOwner(msg, fee, memo, funds),
    options,
  )
}
