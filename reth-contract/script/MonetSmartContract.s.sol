// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import {Script, console} from "forge-std/Script.sol";
import {MonetContract} from "../contracts/MonetSmartContract.sol";

contract SimpleStorageScript is Script {
    MonetContract public monetContract;

    function setUp() public {}

    function run() public {
        vm.startBroadcast();

        monetContract = new MonetContract();

        vm.stopBroadcast();
    }
}
