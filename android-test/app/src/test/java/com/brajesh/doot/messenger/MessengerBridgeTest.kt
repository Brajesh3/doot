package com.brajesh.doot.messenger

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test

class MessengerBridgeTest {

    @Test
    fun testParseContactsJson() {
        val json = """
            [
                {
                    "public_key": "1234567890abcdef",
                    "nickname": "Alice",
                    "unread_count": 2,
                    "last_seen": "2026-09-27T10:00:00Z"
                },
                {
                    "public_key": "fedcba0987654321",
                    "nickname": "Bob",
                    "unread_count": 0
                }
            ]
        """.trimIndent()

        val contacts = MessengerBridge.parseContacts(json)
        assertEquals(2, contacts.size)
        assertEquals("1234567890abcdef", contacts[0].peerKey)
        assertEquals("Alice", contacts[0].nickname)
        assertEquals(2, contacts[0].unreadCount)
        assertEquals("Bob", contacts[1].nickname)
    }

    @Test
    fun testParseMessagesJson() {
        val json = """
            [
                {
                    "id": "msg-1",
                    "conversation_peer": "1234567890abcdef",
                    "sender_name": "Alice",
                    "direction": "Incoming",
                    "content": "Hello Doot!",
                    "timestamp": "12:00:00",
                    "status": "Delivered"
                },
                {
                    "id": "msg-2",
                    "conversation_peer": "1234567890abcdef",
                    "sender_name": "Me",
                    "direction": "Outgoing",
                    "content": "Here is the file",
                    "timestamp": "12:01:00",
                    "status": "Sent",
                    "attachment": {
                        "file_id": "file-123",
                        "file_name": "document.pdf",
                        "file_size": 2048,
                        "mime_type": "application/pdf",
                        "is_directory": false,
                        "local_path": "/path/to/document.pdf"
                    }
                }
            ]
        """.trimIndent()

        val messages = MessengerBridge.parseMessages(json)
        assertEquals(2, messages.size)
        assertEquals("Hello Doot!", messages[0].content)
        assertEquals(false, messages[0].isMine)

        val second = messages[1]
        assertTrue(second.isMine)
        assertNotNull(second.attachment)
        assertEquals("document.pdf", second.attachment?.fileName)
        assertEquals(2048L, second.attachment?.fileSize)
    }

    @Test
    fun testConnectionInfoDisplayLabel() {
        val direct = ConnectionInfo(kind = "Direct", addr = "192.168.1.1:5000", rttMs = 15)
        assertTrue(direct.isDirect())
        assertEquals("🟢 Direct (15ms)", direct.displayLabel())

        val relay = ConnectionInfo(kind = "Relay", url = "https://derp.iroh.network", rttMs = 45)
        assertTrue(relay.isRelayed())
        assertEquals("🟡 Relayed (45ms)", relay.displayLabel())
    }
}
