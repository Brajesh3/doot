package com.example.testapp.messenger

import org.json.JSONArray
import org.json.JSONObject

data class ContactItem(
    val peerKey: String,
    val nickname: String,
    val isOnline: Boolean,
    val unreadCount: Int,
    val lastSeen: String?
)

data class AttachmentItem(
    val fileId: String,
    val fileName: String,
    val fileSize: Long,
    val mimeType: String,
    val isDirectory: Boolean,
    val localPath: String?
)

data class MessageItem(
    val id: String,
    val conversationPeer: String,
    val senderName: String,
    val isMine: Boolean,
    val content: String,
    val timestamp: String,
    val status: String, // Sending, Sent, Delivered, Failed
    val attachment: AttachmentItem? = null
)

data class ConnectionInfo(
    val kind: String, // "Direct", "Relay", "Unknown"
    val addr: String? = null,
    val url: String? = null,
    val rttMs: Long = 0
) {
    fun displayLabel(): String {
        return when (kind) {
            "Direct" -> "🟢 Direct (${rttMs}ms)"
            "Relay" -> "🟡 Relayed (${rttMs}ms)"
            else -> "⚪ Connecting..."
        }
    }
}

object MessengerBridge {
    init {
        try {
            System.loadLibrary("message_jni")
        } catch (e: UnsatisfiedLinkError) {
            e.printStackTrace()
        }
    }

    external fun initEngine(dataDir: String, nickname: String): Boolean
    external fun getMyTicket(): String
    external fun getMyNodeId(): String
    external fun getMyNickname(): String
    external fun connectPeer(ticketOrId: String, nickname: String): Boolean
    external fun sendTextMessage(peerKey: String, content: String): Boolean
    external fun sendFile(peerKey: String, path: String, isDirectory: Boolean): Boolean
    external fun pingPeer(peerKey: String): Boolean
    external fun sendTyping(peerKey: String, isTyping: Boolean): Boolean
    external fun getContactsJson(): String
    external fun getMessagesJson(peerKey: String): String
    external fun pollEventsJson(): String
    external fun markAsRead(peerKey: String): Boolean

    fun parseContacts(jsonStr: String): List<ContactItem> {
        val list = mutableListOf<ContactItem>()
        if (jsonStr.isBlank()) return list
        try {
            val arr = JSONArray(jsonStr)
            for (i in 0 until arr.length()) {
                val obj = arr.getJSONObject(i)
                val pk = if (obj.has("public_key")) obj.getString("public_key") else obj.optString("peer_key", "")
                list.add(
                    ContactItem(
                        peerKey = pk,
                        nickname = obj.optString("nickname", "Peer"),
                        isOnline = true,
                        unreadCount = obj.optInt("unread_count", 0),
                        lastSeen = if (obj.has("last_seen")) obj.getString("last_seen") else null
                    )
                )
            }
        } catch (e: Exception) {
            e.printStackTrace()
        }
        return list
    }

    fun parseMessages(jsonStr: String): List<MessageItem> {
        val list = mutableListOf<MessageItem>()
        if (jsonStr.isBlank()) return list
        try {
            val arr = JSONArray(jsonStr)
            for (i in 0 until arr.length()) {
                val obj = arr.getJSONObject(i)
                val dir = obj.optString("direction", "Outgoing")
                val isMine = dir.equals("Outgoing", ignoreCase = true)
                val status = obj.optString("status", "Sent")

                var attachment: AttachmentItem? = null
                if (obj.has("attachment") && !obj.isNull("attachment")) {
                    val attObj = obj.getJSONObject("attachment")
                    attachment = AttachmentItem(
                        fileId = attObj.optString("file_id", ""),
                        fileName = attObj.optString("file_name", "file"),
                        fileSize = attObj.optLong("file_size", 0L),
                        mimeType = attObj.optString("mime_type", ""),
                        isDirectory = attObj.optBoolean("is_directory", false),
                        localPath = if (attObj.has("local_path") && !attObj.isNull("local_path")) attObj.getString("local_path") else null
                    )
                }

                list.add(
                    MessageItem(
                        id = obj.getString("id"),
                        conversationPeer = obj.getString("conversation_peer"),
                        senderName = obj.optString("sender_name", if (isMine) "Me" else "Peer"),
                        isMine = isMine,
                        content = obj.getString("content"),
                        timestamp = obj.optString("timestamp", ""),
                        status = status,
                        attachment = attachment
                    )
                )
            }
        } catch (e: Exception) {
            e.printStackTrace()
        }
        return list
    }
}
