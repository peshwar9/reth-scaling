// SPDX-License-Identifier: MIT
pragma solidity ^0.8.8;
//import "@openzeppelin/contracts/security/ReentrancyGuard.sol";



abstract contract ReentrancyGuard {
    // Booleans are more expensive than uint256 or any type that takes up a full
    // word because each write operation emits an extra SLOAD to first read the
    // slot's contents, replace the bits taken up by the boolean, and then write
    // back. This is the compiler's defense against contract upgrades and
    // pointer aliasing, and it cannot be disabled.

    // The values being non-zero value makes deployment a bit more expensive,
    // but in exchange the refund on every call to nonReentrant will be lower in
    // amount. Since refunds are capped to a percentage of the total
    // transaction's gas, it is best to keep them low in cases like this one, to
    // increase the likelihood of the full refund coming into effect.
    uint256 private constant NOT_ENTERED = 1;
    uint256 private constant ENTERED = 2;

    uint256 private _status;

    /**
     * @dev Unauthorized reentrant call.
     */
    error ReentrancyGuardReentrantCall();

    constructor() {
        _status = NOT_ENTERED;
    }

    /**
     * @dev Prevents a contract from calling itself, directly or indirectly.
     * Calling a `nonReentrant` function from another `nonReentrant`
     * function is not supported. It is possible to prevent this from happening
     * by making the `nonReentrant` function external, and making it call a
     * `private` function that does the actual work.
     */
    modifier nonReentrant() {
        _nonReentrantBefore();
        _;
        _nonReentrantAfter();
    }

    function _nonReentrantBefore() private {
        // On the first call to nonReentrant, _status will be NOT_ENTERED
        if (_status == ENTERED) {
            revert ReentrancyGuardReentrantCall();
        }

        // Any calls to nonReentrant after this point will fail
        _status = ENTERED;
    }

    function _nonReentrantAfter() private {
        // By storing the original value once again, a refund is triggered (see
        // https://eips.ethereum.org/EIPS/eip-2200)
        _status = NOT_ENTERED;
    }

    /**
     * @dev Returns true if the reentrancy guard is currently set to "entered", which indicates there is a
     * `nonReentrant` function in the call stack.
     */
    function _reentrancyGuardEntered() internal view returns (bool) {
        return _status == ENTERED;
    }
}

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

    // Message ID tracking per destination chain
    mapping(uint32 => uint32) private messageIdByDestinationChain;

    // map to store received message id for source chains
    mapping(uint32 => uint32) private lastProcessedMessageIdBySourceChain;

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
        address recipient,
        uint32 indexed messageId,
        uint256 amount
    );

    event ETHReceivedFromSourceChain(
        uint32 indexed sourceChainId,
        address indexed sourceChainSender,
        address recipient,
        uint32 indexed sourceChainMessageId,
        uint256 amount
    );

    event ETHReceivedFromSourceChainInBatch(
        uint32 indexed sourceChainId,
        address[] recipients,
        uint256[] amounts,
        uint32 startMessageId,
        uint32 indexed endMessageId
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

    // Allow contract to receive ETH directly
    receive() external payable {}

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
        require(msg.value == requiredFee, "Incorrect fee amount");

        messageIdByDestinationChain[chainID]++;
        uint32 messageId = messageIdByDestinationChain[chainID];

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
            sourceChainMessageId >
                lastProcessedMessageIdBySourceChain[sourceChainId],
            "Message ID too old or already processed"
        );

        // Update the last processed message ID for this chain
        lastProcessedMessageIdBySourceChain[
            sourceChainId
        ] = sourceChainMessageId;

        emit MessageReceived(
            sourceChainId,
            sourceChainSender,
            sourceChainMessageId,
            payload
        );
    }

    function sendETHToDestinationChain(uint32 chainID, address recipient)
        external
        payable
    {
        require(
            supportedDestinationChains[chainID].contractAddress != address(0),
            "Chain not supported"
        );
        require(msg.value > 0, "Amount must be greater than zero");
        require(recipient != address(0), "Invalid recipient");

        messageIdByDestinationChain[chainID]++;
        uint32 messageId = messageIdByDestinationChain[chainID];

        emit ETHSentToDestinationChain(
            chainID,
            msg.sender,
            recipient,
            messageId,
            msg.value
        );
    }

    function receiveETHFromSourceChain(
        uint32 sourceChainId,
        address sourceChainSender,
        address recipient,
        uint32 sourceChainMessageId,
        uint256 amount
    ) external onlyWhitelistedRelayers {
        require(
            sourceChainMessageId >
                lastProcessedMessageIdBySourceChain[sourceChainId],
            "Message ID too old or already processed"
        );
        require(
            amount <= address(this).balance,
            "Insufficient contract balance"
        );
        require(amount > 0, "Amount must be greater than zero");
        require(recipient != address(0), "Invalid recipient");

        (bool success, ) = payable(recipient).call{value: amount}("");
        require(success, "ETH transfer failed");

        lastProcessedMessageIdBySourceChain[
            sourceChainId
        ] = sourceChainMessageId;

        emit ETHReceivedFromSourceChain(
            sourceChainId,
            sourceChainSender,
            recipient,
            sourceChainMessageId,
            amount
        );
    }

    // Function to receive ETH in batch from a source chain
    function receiveETHfromSourceChainInBatch(
        uint32 sourceChainId,
        uint32 sourceChainFirstMessageId,
        address[] calldata recipients,
        uint256[] calldata amounts
    ) external onlyWhitelistedRelayers {
        uint256 recipientsLength = recipients.length;
        require(recipientsLength == amounts.length, "Mismatched arrays length");
        require(recipientsLength > 0, "No recipients provided");
        require(
            sourceChainFirstMessageId >
                lastProcessedMessageIdBySourceChain[sourceChainId],
            "Message ID too old or already processed"
        );

        uint32 endMessageId = sourceChainFirstMessageId +
            uint32(recipientsLength) -
            1;

        uint256 totalAmount;
        uint256 contractBalance = address(this).balance;

        for (uint256 i = 0; i < recipientsLength; ) {
            uint256 amount = amounts[i];
            require(amount > 0, "Amount must be greater than zero");
            totalAmount += amount;

            (bool success, ) = payable(recipients[i]).call{value: amount}("");
            require(success, "ETH transfer failed");

            unchecked {
                i++;
            } // Save gas
        }

        require(
            totalAmount <= contractBalance,
            "Insufficient contract balance"
        );

        lastProcessedMessageIdBySourceChain[sourceChainId] = endMessageId;

        emit ETHReceivedFromSourceChainInBatch(
            sourceChainId,
            recipients,
            amounts,
            sourceChainFirstMessageId,
            endMessageId
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
    function getDestinationChainInfo(uint32 destionationChainId)
        external
        view
        returns (
            string memory rpcURL,
            address contractAddress,
            uint8[] memory supportedTypes
        )
    {
        require(
            supportedDestinationChains[destionationChainId].contractAddress !=
                address(0),
            "Chain not supported"
        );

        DestinationChain storage chain = supportedDestinationChains[
            destionationChainId
        ];
        return (chain.rpcURL, chain.contractAddress, chain.supportedTypes);
    }

    // Function to get required fee for a given chain and type
    function getRequiredFeeForDestinationChain(
        uint32 destionationChainId,
        uint8 messageType
    ) external view returns (uint256) {
        require(
            supportedDestinationChains[destionationChainId].contractAddress !=
                address(0),
            "Chain not supported"
        );
        DestinationChain storage chain = supportedDestinationChains[
            destionationChainId
        ];

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

    function getLastProcessedMessageIdBySourceChain(uint32 sourceChainId)
        external
        view
        returns (uint32)
    {
        return lastProcessedMessageIdBySourceChain[sourceChainId];
    }

    function getMessageIdByDestinationChain(uint32 destinationChainId)
        external
        view
        returns (uint32)
    {
        return messageIdByDestinationChain[destinationChainId];
    }
}


