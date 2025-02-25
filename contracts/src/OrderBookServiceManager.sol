// SPDX-License-Identifier: UNLICENSED
pragma solidity >=0.8.13;

import "eigenlayer-middleware/lib/eigenlayer-contracts/src/contracts/libraries/BytesLib.sol";
import "contracts/src/IOrderBookTaskManager.sol";
import "eigenlayer-middleware/src/ServiceManagerBase.sol";

/**
 * @title Primary entrypoint for procuring services from OrderBook.
 * @author Layr Labs, Inc.
 */
contract OrderBookServiceManager is ServiceManagerBase {
    using BytesLib for bytes;

    IOrderBookTaskManager
        public immutable OrderBookTaskManager;

    /// @notice when applied to a function, ensures that the function is only callable by the `registryCoordinator`.
    modifier onlyOrderBookTaskManager() {
        require(
            msg.sender == address(OrderBookTaskManager),
            "onlyOrderBookTaskManager: not from credible order book task manager"
        );
        _;
    }

    constructor(
        IAVSDirectory _avsDirectory,
        IRewardsCoordinator _rewardsCoordinator,
        IRegistryCoordinator _registryCoordinator,
        IStakeRegistry _stakeRegistry,
        IOrderBookTaskManager _OrderBookTaskManager
    )
        ServiceManagerBase(
            _avsDirectory,
            _rewardsCoordinator,
            _registryCoordinator,
            _stakeRegistry
        )
    {
        OrderBookTaskManager = _OrderBookTaskManager;
    }

    /// @notice Called in the event of challenge resolution, in order to forward a call to the Slasher, which 'freezes' the `operator`.
    /// @dev The Slasher contract is under active development and its interface expected to change.
    ///      We recommend writing slashing logic without integrating with the Slasher at this point in time.
    function freezeOperator(
        address operatorAddr
    ) external onlyOrderBookTaskManager {
        // slasher.freezeOperator(operatorAddr);
    }
}
