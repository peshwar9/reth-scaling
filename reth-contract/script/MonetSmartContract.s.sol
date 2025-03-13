// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import {Script, console} from "forge-std/Script.sol";
import {MonetSmartContract} from "../contracts/MonetSmartContract.sol";

contract SimpleStorageScript is Script {
    MonetSmartContract public monetContract;

    function setUp() public {}

    function run() public {
        vm.startBroadcast();

        monetContract = new MonetSmartContract();

        vm.stopBroadcast();
    }
}
