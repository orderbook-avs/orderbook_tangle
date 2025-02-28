// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "openzeppelin-contracts/contracts/token/ERC20/ERC20.sol";

contract TokenB is ERC20 {
    uint256 private constant _initialSupply = 100e12; // 100 trillion tokens
    uint256 private constant _salesTaxRate = 3; // 3%

    constructor() ERC20("TokenB", "TokenB") {
        _mint(address(0x8D5470Dd39eC0933A0CCAEd0652E80ce891c4225), _initialSupply);
    }
}