// SPDX-License-Identifier: Apache-2.0
pragma solidity >=0.8.0;

import {Address} from "@openzeppelin/contracts/utils/Address.sol";
import {HypERC20Collateral} from "../HypERC20Collateral.sol";
import {console} from "forge-std/console.sol";

contract HypERC20CollateralMemo is HypERC20Collateral {
    mapping(address => mapping(uint256 => bytes)) private _memos;
    mapping(address => uint256) private _nonces;
    bytes public testMemo;

    constructor(
        address erc20,
        uint256 _scale,
        address _mailbox
    ) HypERC20Collateral(erc20, _scale, _mailbox) {
        testMemo = "";
    }

    function setMemoForNextTransfer(bytes calldata memo) external {
        _memos[msg.sender][_nonces[msg.sender]] = memo;
    }

    function _transferFromSender(
        uint256 _amount
    ) internal virtual override returns (bytes memory) {
        super._transferFromSender(_amount);
        bytes memory memo = _memos[msg.sender][_nonces[msg.sender]];

        delete _memos[msg.sender][_nonces[msg.sender]];
        _nonces[msg.sender]++;
        testMemo = memo;
        return memo;
    }
}
