// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import {Script, console} from "forge-std/Script.sol";
import {SimpleStorage} from "../contracts/SimpleStorage.sol";

contract SimpleStorageScript is Script {
    SimpleStorage public ssContract;

    function setUp() public {}

    function run() public {
        vm.startBroadcast();

        ssContract = new SimpleStorage();

        vm.stopBroadcast();
    }
}
