package com.example.testapp.messenger

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.json.JSONArray
import org.json.JSONObject

data class MessengerUiState(
    val isInitialized: Boolean = false,
    val myTicket: String = "",
    val myNodeId: String = "",
    val myNickname: String = "Doot Android",
    val contacts: List<ContactItem> = emptyList(),
    val activePeer: ContactItem? = null,
    val messages: List<MessageItem> = emptyList(),
    val connectionTypes: Map<String, ConnectionInfo> = emptyMap(),
    val isPeerTyping: Boolean = false,
    val bannerMessage: String? = null
)

class MessengerViewModel(application: Application) : AndroidViewModel(application) {

    private val _uiState = MutableStateFlow(MessengerUiState())
    val uiState: StateFlow<MessengerUiState> = _uiState.asStateFlow()

    init {
        initializeEngine()
    }

    private fun initializeEngine() {
        viewModelScope.launch(Dispatchers.IO) {
            val dataDir = getApplication<Application>().filesDir.resolve("iroh_messenger").absolutePath
            val success = MessengerBridge.initEngine(dataDir, "Android Phone 📱")
            if (success) {
                val ticket = MessengerBridge.getMyTicket()
                val nodeId = MessengerBridge.getMyNodeId()
                val nick = MessengerBridge.getMyNickname()
                val contacts = MessengerBridge.parseContacts(MessengerBridge.getContactsJson())
                val active = contacts.firstOrNull()
                val msgs = if (active != null) {
                    MessengerBridge.parseMessages(MessengerBridge.getMessagesJson(active.peerKey))
                } else emptyList()

                _uiState.update {
                    it.copy(
                        isInitialized = true,
                        myTicket = ticket,
                        myNodeId = nodeId,
                        myNickname = nick,
                        contacts = contacts,
                        activePeer = active,
                        messages = msgs
                    )
                }

                startEventLoop()
            } else {
                _uiState.update { it.copy(bannerMessage = "Failed to initialize Iroh engine") }
            }
        }
    }

    private fun startEventLoop() {
        viewModelScope.launch(Dispatchers.IO) {
            while (true) {
                try {
                    val eventsJson = MessengerBridge.pollEventsJson()
                    if (eventsJson.isNotBlank() && eventsJson != "[]") {
                        handleEvents(eventsJson)
                    }
                } catch (e: Exception) {
                    e.printStackTrace()
                }
                delay(120)
            }
        }
    }

    private fun handleEvents(eventsJson: String) {
        val arr = JSONArray(eventsJson)
        var refreshContacts = false
        var refreshMessages = false

        for (i in 0 until arr.length()) {
            val evt = arr.getJSONObject(i)
            val type = evt.optString("type", "")
            when {
                type == "EngineReady" || evt.has("EngineReady") -> {
                    refreshContacts = true
                }
                type == "PeerConnected" || evt.has("PeerConnected") -> {
                    refreshContacts = true
                    val pk = evt.optString("peer_key", "")
                    val ctObj = evt.optJSONObject("connection_type")
                    if (ctObj != null && pk.isNotBlank()) {
                        val kind = ctObj.optString("kind", "Unknown")
                        val addr = ctObj.optString("addr", "")
                        val url = ctObj.optString("url", "")
                        val rtt = ctObj.optLong("rtt_ms", 0L)
                        val info = ConnectionInfo(kind, addr, url, rtt)
                        _uiState.update { it.copy(connectionTypes = it.connectionTypes + (pk to info)) }
                    }
                    _uiState.update { it.copy(bannerMessage = "Peer connected via QUIC!") }
                }
                type == "ConnectionInfoUpdated" || evt.has("ConnectionInfoUpdated") -> {
                    val pk = evt.optString("peer_key", "")
                    val ctObj = evt.optJSONObject("connection_type")
                    if (ctObj != null && pk.isNotBlank()) {
                        val kind = ctObj.optString("kind", "Unknown")
                        val addr = ctObj.optString("addr", "")
                        val url = ctObj.optString("url", "")
                        val rtt = ctObj.optLong("rtt_ms", 0L)
                        val info = ConnectionInfo(kind, addr, url, rtt)
                        _uiState.update { it.copy(connectionTypes = it.connectionTypes + (pk to info)) }
                    }
                }
                type == "PeerDisconnected" || evt.has("PeerDisconnected") -> {
                    refreshContacts = true
                    val pk = evt.optString("peer_key", "")
                    if (pk.isNotBlank()) {
                        _uiState.update { it.copy(connectionTypes = it.connectionTypes - pk) }
                    }
                }
                type == "PeerTyping" || evt.has("PeerTyping") -> {
                    _uiState.update { it.copy(isPeerTyping = true) }
                }
                type == "MessageReceived" || evt.has("MessageReceived") -> {
                    refreshContacts = true
                    refreshMessages = true
                }
                type == "MessageSent" || evt.has("MessageSent") -> {
                    refreshContacts = true
                    refreshMessages = true
                }
                type == "ContactListUpdated" || evt.has("ContactListUpdated") -> {
                    refreshContacts = true
                }
                type == "FileTransferProgress" || evt.has("FileTransferProgress") -> {
                    refreshMessages = true
                }
                type == "FileTransferComplete" || evt.has("FileTransferComplete") -> {
                    refreshMessages = true
                    refreshContacts = true
                    val isOutgoing = evt.optBoolean("is_outgoing", false)
                    if (!isOutgoing) {
                        setBanner("File received and verified over Iroh QUIC!")
                    }
                }
                type == "Error" || evt.has("Error") -> {
                    val errMsg = evt.optString("error", "Connection error")
                    setBanner("Notice: $errMsg")
                }
                type == "MessageStatusUpdated" || evt.has("MessageStatusUpdated") -> {
                    refreshMessages = true
                }
            }
        }

        if (refreshContacts) {
            val contacts = MessengerBridge.parseContacts(MessengerBridge.getContactsJson())
            _uiState.update { current ->
                val active = current.activePeer ?: contacts.firstOrNull()
                current.copy(contacts = contacts, activePeer = active)
            }
            if (_uiState.value.activePeer != null) {
                refreshMessages = true
            }
        }

        if (refreshMessages) {
            val active = _uiState.value.activePeer
            if (active != null) {
                val msgs = MessengerBridge.parseMessages(MessengerBridge.getMessagesJson(active.peerKey))
                _uiState.update { it.copy(messages = msgs, isPeerTyping = false) }
            }
        }
    }

    private fun setBanner(msg: String) {
        _uiState.update { it.copy(bannerMessage = msg) }
        viewModelScope.launch {
            delay(5000)
            _uiState.update { if (it.bannerMessage == msg) it.copy(bannerMessage = null) else it }
        }
    }

    fun selectPeer(contact: ContactItem) {
        viewModelScope.launch(Dispatchers.IO) {
            MessengerBridge.markAsRead(contact.peerKey)
            val msgs = MessengerBridge.parseMessages(MessengerBridge.getMessagesJson(contact.peerKey))
            val contacts = MessengerBridge.parseContacts(MessengerBridge.getContactsJson())
            _uiState.update {
                it.copy(
                    activePeer = contact,
                    messages = msgs,
                    contacts = contacts,
                    isPeerTyping = false
                )
            }
        }
    }

    fun connectPeer(ticketOrId: String, nickname: String) {
        viewModelScope.launch(Dispatchers.IO) {
            val trimmed = ticketOrId.trim()
            if (trimmed.isEmpty()) return@launch

            setBanner("Connecting to peer...")
            val ok = MessengerBridge.connectPeer(trimmed, nickname.trim())
            if (ok) {
                delay(150)
                val contacts = MessengerBridge.parseContacts(MessengerBridge.getContactsJson())
                val peer = contacts.find { it.nickname == nickname.trim() } ?: contacts.firstOrNull()
                _uiState.update {
                    it.copy(
                        contacts = contacts,
                        activePeer = peer ?: it.activePeer
                    )
                }
            } else {
                setBanner("Invalid ticket format")
            }
        }
    }

    fun sendMessage(text: String) {
        val trimmed = text.trim()
        val active = _uiState.value.activePeer ?: return
        if (trimmed.isEmpty()) return

        viewModelScope.launch(Dispatchers.IO) {
            MessengerBridge.sendTextMessage(active.peerKey, trimmed)
            delay(50)
            val msgs = MessengerBridge.parseMessages(MessengerBridge.getMessagesJson(active.peerKey))
            val contacts = MessengerBridge.parseContacts(MessengerBridge.getContactsJson())
            _uiState.update { it.copy(messages = msgs, contacts = contacts) }
        }
    }

    fun sendTyping(isTyping: Boolean) {
        val active = _uiState.value.activePeer ?: return
        viewModelScope.launch(Dispatchers.IO) {
            MessengerBridge.sendTyping(active.peerKey, isTyping)
        }
    }

    fun sendFile(path: String, isDirectory: Boolean = false) {
        val active = _uiState.value.activePeer ?: return
        viewModelScope.launch(Dispatchers.IO) {
            val ok = MessengerBridge.sendFile(active.peerKey, path, isDirectory)
            if (ok) {
                delay(80)
                val msgs = MessengerBridge.parseMessages(MessengerBridge.getMessagesJson(active.peerKey))
                val contacts = MessengerBridge.parseContacts(MessengerBridge.getContactsJson())
                _uiState.update { it.copy(messages = msgs, contacts = contacts) }
            } else {
                setBanner("Failed to send ${if (isDirectory) "folder" else "file"}")
            }
        }
    }

    fun pingPeer(peerKey: String) {
        viewModelScope.launch(Dispatchers.IO) {
            MessengerBridge.pingPeer(peerKey)
        }
    }

    fun dismissBanner() {
        _uiState.update { it.copy(bannerMessage = null) }
    }
}
