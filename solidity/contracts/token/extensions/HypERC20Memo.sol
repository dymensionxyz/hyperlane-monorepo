// SPDX-License-Identifier: Apache-2.0
pragma solidity >=0.8.0;

import {Address} from "@openzeppelin/contracts/utils/Address.sol";
import {HypERC20} from "../HypERC20.sol";
import {FungibleTokenRouter} from "../libs/FungibleTokenRouter.sol";
import {ERC20Upgradeable} from "@openzeppelin/contracts-upgradeable/token/ERC20/ERC20Upgradeable.sol";
import {console} from "forge-std/console.sol";
import {TokenRouter} from "../libs/TokenRouter.sol";

// We have to copy and change the original code because the original _transferFromSender method is internal and not virtual
contract HypERC20Memo is ERC20Upgradeable, FungibleTokenRouter {
    event IncludedMemo(bytes memo);

    mapping(address => mapping(uint256 => bytes)) private _memos;
    mapping(address => uint256) private _nonces;
    /// ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    uint8 private immutable _decimals;

    function setMemoForNextTransfer(bytes calldata memo) external {
        _memos[msg.sender][_nonces[msg.sender]] = memo;
    }

    constructor(
        uint8 __decimals,
        uint256 _scale,
        address _mailbox
    ) FungibleTokenRouter(_scale, _mailbox) {
        _decimals = __decimals;
    }

    /**
     * @notice Initializes the Hyperlane router, ERC20 metadata, and mints initial supply to deployer.
     * @param _totalSupply The initial supply of the token.
     * @param _name The name of the token.
     * @param _symbol The symbol of the token.
     */
    function initialize(
        uint256 _totalSupply,
        string memory _name,
        string memory _symbol,
        address _hook,
        address _interchainSecurityModule,
        address _owner
    ) public virtual initializer {
        // Initialize ERC20 metadata
        __ERC20_init(_name, _symbol);
        _mint(msg.sender, _totalSupply);
        _MailboxClient_initialize(_hook, _interchainSecurityModule, _owner);
    }

    function decimals() public view virtual override returns (uint8) {
        return _decimals;
    }

    function balanceOf(
        address _account
    )
        public
        view
        virtual
        override(TokenRouter, ERC20Upgradeable)
        returns (uint256)
    {
        return ERC20Upgradeable.balanceOf(_account);
    }

    /**
     * @dev Burns `_amount` of token from `msg.sender` balance.
     * @inheritdoc TokenRouter
     */
    function _transferFromSender(
        uint256 _amount
    ) internal override returns (bytes memory) {
        _burn(msg.sender, _amount);
        bytes memory memo = _memos[msg.sender][_nonces[msg.sender]];

        delete _memos[msg.sender][_nonces[msg.sender]];
        _nonces[msg.sender]++;
        emit IncludedMemo(memo);
        return memo;
    }

    /**
     * @dev Mints `_amount` of token to `_recipient` balance.
     * @inheritdoc TokenRouter
     */
    function _transferTo(
        address _recipient,
        uint256 _amount,
        bytes calldata // no metadata
    ) internal virtual override {
        _mint(_recipient, _amount);
    }
}
