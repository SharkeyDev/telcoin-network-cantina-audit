// SPDX-License-Identifier: MIT or Apache-2.0

pragma solidity ^0.8.0;

import { Proxy } from '@axelar-network/axelar-gmp-sdk-solidity/contracts/upgradable/Proxy.sol';

/**
 * @title AxelarGasServiceProxy
 * @notice Minimal override of `Proxy` with named contract identifier
 * @dev An older version (0.8.9) of this proxy exists in @axelar-network/axelar-cgp-solidity
 */
contract AxelarGasServiceProxy is Proxy {

    constructor(
        address implementationAddress,
        address owner,
        bytes memory setupParams
    ) Proxy(implementationAddress, owner, setupParams) {}
    
    function contractId() internal pure override returns (bytes32) {
        return keccak256('axelar-gas-service');
    }
}
