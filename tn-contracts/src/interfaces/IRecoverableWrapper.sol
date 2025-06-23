/**
 * SPDX-License-Identifier: MIT or Apache-2.0
 *
 * Minimal fork of the IRecoverableWrapper developed in 2023 by Circle Internet Financial, LTD.
 * Modifications have been made to the original codebase, primarily removal of unused features.
 *
 * Original information:
 * Author: Circle Internet Financial, LTD
 * License: Apache License, Version 2.0
 * Source: https://github.com/circlefin/recoverable-wrapper
 *
 * Unless required by applicable law or agreed to in writing, this software is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See LICENSE files for specific language, permissions & limitations.
 */
pragma solidity ^0.8.20;

import { Record } from "../recoverable-wrapper/RecordUtil.sol";

/**
 * @dev Interface of the ERC20R standard.
 */
interface IRecoverableWrapper {
    /**
     * @dev Emitted when a transfer occurs. Could be a settled transfer or
     * an unsettled transfer.
     * @param from - sender
     * @param to - receiver
     * @param unsettledTransferred - amount of sender's unsettled funds transferred. 0 if includeUnsettled was set to
     * false.
     * @param settledTransferred - amount of sender's settled funds transferred. Could be 0 if includeUnsettled was
     * true.
     * @param rawIndex - only needed if this transfer is frozen later. This
     * is the index of the transfer record in memory, which is only deleted after
     * the funds are settled.
     */
    event Transfer(
        address indexed from,
        address indexed to,
        uint256 unsettledTransferred,
        uint256 settledTransferred,
        uint256 rawIndex
    );

    /**
     * @dev Emitted when an account is disabled from unwrapping.
     * @param account disabled
     */
    event UnwrapDisabled(address indexed account);

    /**
     *
     * @param dst - the account trying to wrap his own tokens
     * @param amount to wrap
     */
    event Wrap(address indexed dst, uint256 amount);

    /**
     *
     * @param src - account trying to unwrap tokens back to base token
     * @param amount to unwrap
     */
    event Unwrap(address indexed src, address indexed to, uint256 amount);

    /**
     * @dev returns the address of the base token this contract wraps.
     */
    function baseToken() external view returns (address);

    /**
     * @dev returns the window of recovery.
     */
    function recoverableWindow() external view returns (uint256);

    /**
     * @dev Returns the amount of tokens owned by `account`. If includeUnsettled
     * is false, then it returns the settled balance.
     */
    function balanceOf(address account, bool includeUnsettled) external view returns (uint256);

    /**
     * @notice Fetches the account's outstanding unsettled records
     * @dev Intended as a convenience function only, eg for frontends. Does not prevent
     * reverts arising from unbounded storage access
     */
    function unsettledRecords(address account) external view returns (Record[] memory);

    /**
     * Each time an account receives ERC20R tokens, this nonce increments. This may be
     * useful in contexts where another party wants to evaluate the clawback risk of an account's
     * tokens, based on the account's current state.
     * @param account to retrieve nonce for.
     */
    function nonce(address account) external view returns (uint128);

    /**
     * Allows caller to wrap their own base tokens
     * @param amount to wrap
     */
    function wrap(uint256 amount) external;

    /**
     * Allows caller to unwrap their own recoverable tokens back to the base token
     * @param amount to unwrap
     */
    function unwrap(uint256 amount) external;

    /**
     * First unwraps caller's recoverable tokens and then sends base token to another address
     * @param amount to unwrap
     * @param to - the address to send unwrapped tokens to
     */
    function unwrapTo(address to, uint256 amount) external;

    /**
     * @dev Moves `amount` tokens from the caller's account to `to`.
     * if `includeUnsettled` is false, it will only transfer out of settled funds.
     * Returns a boolean value indicating whether the operation succeeded.
     *
     * Emits a {TransferSettled} event, or both {TransferSettled} and {TransferUnsettled} events.
     */
    function transfer(address to, uint256 amount, bool includeUnsettled) external returns (bool);

    /**
     * @dev Moves `amount` tokens from `from` to `to` using the
     * allowance mechanism. `amount` is then deducted from the caller's
     * allowance. if `includeUnsettled` is false, the allowance must also be settled-only.
     *
     * Returns a boolean value indicating whether the operation succeeded.
     *
     * Emits a {Transfer} event.
     */
    function transferFrom(address from, address to, uint256 amount, bool includeUnsettled) external returns (bool);
}
