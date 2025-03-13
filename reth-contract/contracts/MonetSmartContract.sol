// SPDX-License-Identifier: MIT
pragma solidity ^0.8.8;
import "@openzeppelin/contracts/security/ReentrancyGuard.sol";

contract MonetSmartContract is ReentrancyGuard {
    // Owner of the contract (admin role)
    address public owner;

    // Relayer whitelist mapping
    mapping(address => bool) public relayerWhitelistMap; // map to enable/disable relayer
    address[] private relayerWhitelistArray; // array to store all relayers

    // Structure to hold supported chain details
    struct DestinationChain {
        string rpcURL;
        mapping(uint8 => uint256) fees; // Fees mapped by transaction type (0-3)
        uint8[] supportedTypes; // Array to track supported message types
        address contractAddress;
    }

    // Mapping to store supported destination chains by their unique chainID
    mapping(uint32 => DestinationChain) public supportedDestinationChains;

    // Array to store all destination chain IDs
    uint32[] private destinationChainIDs;

    // Auto-incrementing message ID
    uint32 public nextMessageId = 1;
    // map to store received message id for source chains
    mapping(uint32 => uint32) private lastProcessedMessageId;

    // Events
    event DestinationChainAdded(
        uint32 indexed chainID,
        string rpcURL,
        address contractAddress,
        uint8[] messageTypes,
        uint256[] fees
    );
    event DestinationChainRemoved(uint32 indexed chainID);
    event DestinationChainUpdated(
        uint32 indexed chainID,
        string rpcURL,
        address contractAddress,
        uint8[] messageTypes,
        uint256[] fees
    );
    event OwnershipTransferred(
        address indexed previousOwner,
        address indexed newOwner
    );
    event MessageSent(
        uint32 indexed chainID,
        uint32 indexed messageId,
        address indexed sender,
        uint8 messageType,
        bytes payload,
        uint256 feePaid
    );
    event MessageReceived(
        uint32 indexed sourceChainId,
        address indexed sourceChainSender,
        uint32 indexed sourceChainMessageId,
        bytes payload
    );

    event ETHSentToDestinationChain(
        uint32 indexed chainID,
        address indexed sender,
        uint32 indexed messageId,
        uint256 amount
    );

    event ETHReceivedFromSourceChain(
        uint32 indexed sourceChainId,
        address indexed sourceChainSender,
        uint32 indexed sourceChainMessageId,
        uint256 amount
    );

    event FundsWithdrawn(address indexed owner, uint256 amount);
    event RelayerAddressAddedToWhitelist(address indexed _address);
    event RelayerAddressRemovedFromWhitelist(address indexed _address);

    // Modifier to restrict access to only the contract owner
    modifier onlyOwner() {
        require(msg.sender == owner, "Only owner can perform this action");
        _;
    }

    // Modifier to restrict access to only whitelisted relayers
    modifier onlyWhitelistedRelayers() {
        require(relayerWhitelistMap[msg.sender], "Address is not whitelisted");
        _;
    }

    // Constructor
    constructor() {
        owner = msg.sender;
        emit OwnershipTransferred(address(0), msg.sender);
    }

    // Function to add a relayer to the whitelist
    function addRelayerAddressToWhitelist(address _address) external onlyOwner {
        require(
            !relayerWhitelistMap[_address],
            "Address is already whitelisted"
        );
        relayerWhitelistMap[_address] = true;
        relayerWhitelistArray.push(_address);
        emit RelayerAddressAddedToWhitelist(_address);
    }

    // Function to remove a relayer from the whitelist
    function removeRelayerAddressFromWhitelist(address _address)
        external
        onlyOwner
    {
        require(
            relayerWhitelistMap[_address],
            "Address is not in the whitelist"
        );
        relayerWhitelistMap[_address] = false;

        for (uint256 i = 0; i < relayerWhitelistArray.length; i++) {
            if (relayerWhitelistArray[i] == _address) {
                relayerWhitelistArray[i] = relayerWhitelistArray[
                    relayerWhitelistArray.length - 1
                ];
                relayerWhitelistArray.pop();
                break;
            }
        }
        emit RelayerAddressRemovedFromWhitelist(_address);
    }

    // Function to add a new supported destination chain with fees per transaction type
    function addDestinationChain(
        uint32 chainID,
        string memory rpcURL,
        address contractAddress,
        uint8[] memory types,
        uint256[] memory fees
    ) external onlyOwner {
        require(
            supportedDestinationChains[chainID].contractAddress == address(0),
            "Chain already exists"
        );
        require(
            types.length == fees.length,
            "Mismatch between types and fees length"
        );

        DestinationChain storage chain = supportedDestinationChains[chainID];
        chain.rpcURL = rpcURL;
        chain.contractAddress = contractAddress;
        chain.supportedTypes = types;

        for (uint8 i = 0; i < types.length; i++) {
            chain.fees[types[i]] = fees[i];
        }

        destinationChainIDs.push(chainID);
        emit DestinationChainAdded(
            chainID,
            rpcURL,
            contractAddress,
            types,
            fees
        );
    }

    // Function to update fees for an existing destination chain
    function updateDestinationChain(
        uint32 chainID,
        string memory rpcURL,
        address contractAddress,
        uint8[] memory types,
        uint256[] memory fees
    ) external onlyOwner {
        require(
            supportedDestinationChains[chainID].contractAddress != address(0),
            "Chain does not exist"
        );
        require(
            types.length == fees.length,
            "Mismatch between types and fees length"
        );

        DestinationChain storage chain = supportedDestinationChains[chainID];
        chain.rpcURL = rpcURL;
        chain.contractAddress = contractAddress;
        chain.supportedTypes = types; // Update supported types

        for (uint8 i = 0; i < types.length; i++) {
            chain.fees[types[i]] = fees[i];
        }

        emit DestinationChainUpdated(
            chainID,
            rpcURL,
            contractAddress,
            types,
            fees
        );
    }

    // Function to remove a supported destination chain
    function removeDestinationChain(uint32 chainID) external onlyOwner {
        require(
            supportedDestinationChains[chainID].contractAddress != address(0),
            "Chain does not exist"
        );

        delete supportedDestinationChains[chainID];

        for (uint256 i = 0; i < destinationChainIDs.length; i++) {
            if (destinationChainIDs[i] == chainID) {
                destinationChainIDs[i] = destinationChainIDs[
                    destinationChainIDs.length - 1
                ];
                destinationChainIDs.pop();
                break;
            }
        }
        emit DestinationChainRemoved(chainID);
    }

    // Function to send a message to a destination chain with a specific type
    function sendMessageToDestinationChain(
        uint32 chainID,
        uint8 messageType,
        bytes calldata payload
    ) external payable {
        require(
            supportedDestinationChains[chainID].contractAddress != address(0),
            "Chain not supported"
        );
        uint256 requiredFee = supportedDestinationChains[chainID].fees[
            messageType
        ];
        require(requiredFee > 0, "Message type not supported");
        require(msg.value == requiredFee, "Incorrect fee amount");

        uint32 messageId = nextMessageId;
        nextMessageId++;

        emit MessageSent(
            chainID,
            messageId,
            msg.sender,
            messageType,
            payload,
            requiredFee
        );
    }

    // Function to receive a message (Only whitelisted relayers)
    function receiveMessageFromSourceChain(
        uint32 sourceChainId,
        address sourceChainSender,
        uint32 sourceChainMessageId,
        bytes calldata payload
    ) external onlyWhitelistedRelayers {
        require(
            sourceChainMessageId > lastProcessedMessageId[sourceChainId],
            "Message ID too old or already processed"
        );

        // Update the last processed message ID for this chain
        lastProcessedMessageId[sourceChainId] = sourceChainMessageId;

        emit MessageReceived(
            sourceChainId,
            sourceChainSender,
            sourceChainMessageId,
            payload
        );
    }

    function sendETHToDestinationChain(uint32 chainID) external payable {
        require(
            supportedDestinationChains[chainID].contractAddress != address(0),
            "Chain not supported"
        );
        require(msg.value > 0, "Amount must be greater than zero");

        uint32 messageId = nextMessageId;
        nextMessageId++;

        emit ETHSentToDestinationChain(
            chainID,
            msg.sender,
            messageId,
            msg.value
        );
    }

    function receiveETHFromSourceChain(
        uint32 sourceChainId,
        address sourceChainSender,
        uint32 sourceChainMessageId,
        uint256 amount
    ) external onlyWhitelistedRelayers {
        require(
            sourceChainMessageId > lastProcessedMessageId[sourceChainId],
            "Message ID too old or already processed"
        );
        require(
            amount <= address(this).balance,
            "Insufficient contract balance"
        );
        require(amount > 0, "Amount must be greater than zero");

        (bool success, ) = payable(sourceChainSender).call{value: amount}("");
        require(success, "ETH transfer failed");

        lastProcessedMessageId[sourceChainId] = sourceChainMessageId;

        emit ETHReceivedFromSourceChain(
            sourceChainId,
            sourceChainSender,
            sourceChainMessageId,
            amount
        );
    }

    // Function to withdraw collected funds (Only Owner)
    function withdrawFunds() external onlyOwner nonReentrant {
        uint256 balance = address(this).balance;
        require(balance > 0, "No funds to withdraw");
        (bool success, ) = payable(owner).call{value: balance}("");
        require(success, "Transfer failed");
        emit FundsWithdrawn(owner, balance);
    }

    // Function to get contract balance
    function getContractBalance() external view returns (uint256) {
        return address(this).balance;
    }

    // Function to get all whitelisted relayers
    function getAllWhitelistedRelayers()
        external
        view
        returns (address[] memory)
    {
        return relayerWhitelistArray;
    }

    // Function to get all supported destination chain IDs
    function getAllSupportedDestinationChains()
        external
        view
        returns (uint32[] memory)
    {
        return destinationChainIDs;
    }

    // Function to get destination chain details (RPC URL, contract address, supported message types)
    function getDestinationChainInfo(uint32 chainID)
        external
        view
        returns (
            string memory rpcURL,
            address contractAddress,
            uint8[] memory supportedTypes
        )
    {
        require(
            supportedDestinationChains[chainID].contractAddress != address(0),
            "Chain not supported"
        );

        DestinationChain storage chain = supportedDestinationChains[chainID];
        return (chain.rpcURL, chain.contractAddress, chain.supportedTypes);
    }

    // Function to get required fee for a given chain and type
    function getRequiredFeeForDestinationChain(
        uint32 chainID,
        uint8 messageType
    ) external view returns (uint256) {
        require(
            supportedDestinationChains[chainID].contractAddress != address(0),
            "Chain not supported"
        );
        DestinationChain storage chain = supportedDestinationChains[chainID];

        // Ensure type exists
        bool typeExists = false;
        for (uint8 i = 0; i < chain.supportedTypes.length; i++) {
            if (chain.supportedTypes[i] == messageType) {
                typeExists = true;
                break;
            }
        }
        require(typeExists, "Message type not supported");

        return chain.fees[messageType];
    }
}