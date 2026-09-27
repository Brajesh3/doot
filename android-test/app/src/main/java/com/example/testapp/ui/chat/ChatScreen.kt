package com.example.testapp.ui.chat

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.provider.OpenableColumns
import android.widget.Toast
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.core.content.FileProvider
import com.example.testapp.messenger.AttachmentItem
import com.example.testapp.messenger.ConnectionInfo
import com.example.testapp.messenger.ContactItem
import com.example.testapp.messenger.MessageItem
import com.example.testapp.messenger.MessengerUiState
import com.example.testapp.messenger.MessengerViewModel

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ChatScreen(viewModel: MessengerViewModel) {
    val state by viewModel.uiState.collectAsState()
    val context = LocalContext.current
    var showConnectDialog by remember { mutableStateOf(false) }
    var showConnDialog by remember { mutableStateOf(false) }
    var inputText by remember { mutableStateOf("") }
    val listState = rememberLazyListState()

    val filePickerLauncher = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.GetContent()
    ) { uri: Uri? ->
        uri?.let {
            val path = copyUriToCache(context, it)
            if (path != null) {
                viewModel.sendFile(path, isDirectory = false)
            } else {
                Toast.makeText(context, "Could not read selected file", Toast.LENGTH_SHORT).show()
            }
        }
    }

    // Auto scroll to bottom on new messages
    LaunchedEffect(state.messages.size) {
        if (state.messages.isNotEmpty()) {
            listState.animateScrollToItem(state.messages.size - 1)
        }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = {
                    Column {
                        Text(
                            "🕊 Doot (दूत)",
                            fontWeight = FontWeight.Bold,
                            fontSize = 18.sp,
                            color = Color(0xFF93C5FD)
                        )
                        val shortId = if (state.myNodeId.length > 8) state.myNodeId.take(8) else state.myNodeId
                        Text(
                            "● ${state.myNickname} ($shortId)",
                            fontSize = 12.sp,
                            color = Color(0xFF4ADE80)
                        )
                    }
                },
                actions = {
                    Button(
                        onClick = {
                            if (state.myTicket.isNotEmpty()) {
                                val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
                                val clip = ClipData.newPlainText("Doot Ticket", state.myTicket)
                                clipboard.setPrimaryClip(clip)
                                Toast.makeText(context, "Doot ticket copied to clipboard!", Toast.LENGTH_SHORT).show()
                            }
                        },
                        colors = ButtonDefaults.buttonColors(containerColor = Color(0xFF1E293B)),
                        contentPadding = PaddingValues(horizontal = 10.dp, vertical = 6.dp)
                    ) {
                        Text("📋 Ticket", fontSize = 12.sp, color = Color(0xFFE2E8F0))
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = Color(0xFF0F172A)
                )
            )
        },
        containerColor = Color(0xFF090D16)
    ) { padding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
                .background(Color(0xFF090D16))
        ) {
            // Banner notification
            state.bannerMessage?.let { banner ->
                Surface(
                    color = Color(0xFF1E3A8A),
                    modifier = Modifier.fillMaxWidth()
                ) {
                    Row(
                        modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.SpaceBetween
                    ) {
                        Text(banner, color = Color.White, fontSize = 13.sp)
                        Text(
                            "✕",
                            color = Color.White,
                            fontSize = 14.sp,
                            modifier = Modifier
                                .clickable { viewModel.dismissBanner() }
                                .padding(4.dp)
                        )
                    }
                }
            }

            // Peers Bar (+ New Chat & Contact list)
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .background(Color(0xFF111827))
                    .padding(8.dp),
                verticalAlignment = Alignment.CenterVertically
            ) {
                // Add Contact Chip
                Surface(
                    shape = RoundedCornerShape(16.dp),
                    color = Color(0xFF2563EB),
                    modifier = Modifier
                        .clickable { showConnectDialog = true }
                        .padding(end = 8.dp)
                ) {
                    Text(
                        "+ New Chat",
                        color = Color.White,
                        fontWeight = FontWeight.Medium,
                        fontSize = 13.sp,
                        modifier = Modifier.padding(horizontal = 12.dp, vertical = 6.dp)
                    )
                }

                // Horizontal Contacts
                LazyRow(
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    items(state.contacts) { contact ->
                        val isSelected = state.activePeer?.peerKey == contact.peerKey
                        Surface(
                            shape = RoundedCornerShape(16.dp),
                            color = if (isSelected) Color(0xFF1E40AF) else Color(0xFF1F2937),
                            modifier = Modifier
                                .border(
                                    width = if (isSelected) 1.dp else 0.dp,
                                    color = if (isSelected) Color(0xFF60A5FA) else Color.Transparent,
                                    shape = RoundedCornerShape(16.dp)
                                )
                                .clickable { viewModel.selectPeer(contact) }
                        ) {
                            Row(
                                modifier = Modifier.padding(horizontal = 10.dp, vertical = 6.dp),
                                verticalAlignment = Alignment.CenterVertically
                            ) {
                                Box(
                                    modifier = Modifier
                                        .size(8.dp)
                                        .clip(CircleShape)
                                        .background(if (contact.isOnline) Color(0xFF22C55E) else Color(0xFF6B7280))
                                )
                                Spacer(modifier = Modifier.width(6.dp))
                                Text(
                                    contact.nickname,
                                    color = Color.White,
                                    fontSize = 13.sp,
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis
                                )
                            }
                        }
                    }
                }
            }

            // Active Peer Header Bar (Real-time connection type & RTT)
            state.activePeer?.let { active ->
                val connInfo = state.connectionTypes[active.peerKey]
                val badgeBg = when (connInfo?.kind) {
                    "Direct" -> Color(0xFF14532D)
                    "Relay" -> Color(0xFF713F12)
                    else -> Color(0xFF1E293B)
                }
                val badgeText = when (connInfo?.kind) {
                    "Direct" -> Color(0xFF4ADE80)
                    "Relay" -> Color(0xFFFDE047)
                    else -> Color(0xFF94A3B8)
                }

                Surface(
                    color = Color(0xFF131D31),
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 8.dp, vertical = 4.dp),
                    shape = RoundedCornerShape(10.dp)
                ) {
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(horizontal = 10.dp, vertical = 6.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.SpaceBetween
                    ) {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Box(
                                modifier = Modifier
                                    .size(32.dp)
                                    .clip(CircleShape)
                                    .background(Color(0xFF2563EB)),
                                contentAlignment = Alignment.Center
                            ) {
                                Text(
                                    active.nickname.take(1).uppercase(),
                                    color = Color.White,
                                    fontWeight = FontWeight.Bold,
                                    fontSize = 14.sp
                                )
                            }
                            Spacer(modifier = Modifier.width(8.dp))
                            Column {
                                Text(
                                    active.nickname,
                                    color = Color.White,
                                    fontWeight = FontWeight.SemiBold,
                                    fontSize = 13.sp
                                )
                                val shortKey = if (active.peerKey.length > 12) active.peerKey.take(12) + "…" else active.peerKey
                                Text(
                                    shortKey,
                                    color = Color(0xFF94A3B8),
                                    fontSize = 10.sp
                                )
                            }
                        }

                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Surface(
                                shape = RoundedCornerShape(12.dp),
                                color = badgeBg,
                                modifier = Modifier
                                    .clickable { showConnDialog = true }
                                    .padding(end = 6.dp)
                            ) {
                                Text(
                                    connInfo?.displayLabel() ?: "⚪ Connecting...",
                                    color = badgeText,
                                    fontSize = 11.sp,
                                    fontWeight = FontWeight.Medium,
                                    modifier = Modifier.padding(horizontal = 8.dp, vertical = 4.dp)
                                )
                            }

                            Button(
                                onClick = {
                                    viewModel.pingPeer(active.peerKey)
                                    Toast.makeText(context, "⚡ Pinging peer...", Toast.LENGTH_SHORT).show()
                                },
                                shape = RoundedCornerShape(12.dp),
                                colors = ButtonDefaults.buttonColors(containerColor = Color(0xFF1E293B)),
                                contentPadding = PaddingValues(horizontal = 8.dp, vertical = 4.dp),
                                modifier = Modifier.height(28.dp)
                            ) {
                                Text("⚡ Ping", fontSize = 11.sp, color = Color(0xFF93C5FD))
                            }
                        }
                    }
                }

                if (showConnDialog) {
                    AlertDialog(
                        onDismissRequest = { showConnDialog = false },
                        title = { Text("Connection Diagnostic", color = Color.White, fontWeight = FontWeight.Bold) },
                        text = {
                            Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                                Text("Protocol: QUIC / Iroh Multiplexed", color = Color(0xFF93C5FD), fontSize = 13.sp)
                                Text("Connection Type: ${connInfo?.kind ?: "Unknown"}", color = Color.White, fontSize = 13.sp)
                                if (!connInfo?.addr.isNullOrBlank()) {
                                    Text("Direct Socket: ${connInfo.addr}", color = Color(0xFFCBD5E1), fontSize = 12.sp)
                                }
                                if (!connInfo?.url.isNullOrBlank()) {
                                    Text("Relay DERP: ${connInfo.url}", color = Color(0xFFCBD5E1), fontSize = 12.sp)
                                }
                                Text("RTT Latency: ${connInfo?.rttMs ?: 0} ms", color = Color(0xFF4ADE80), fontSize = 12.sp)
                            }
                        },
                        confirmButton = {
                            TextButton(onClick = { showConnDialog = false }) {
                                Text("Close", color = Color(0xFF60A5FA))
                            }
                        },
                        containerColor = Color(0xFF1E293B)
                    )
                }
            }

            // Main Chat Area
            Box(
                modifier = Modifier
                    .weight(1f)
                    .fillMaxWidth()
            ) {
                if (state.activePeer == null) {
                    Column(
                        modifier = Modifier
                            .fillMaxSize()
                            .padding(32.dp),
                        verticalArrangement = Arrangement.Center,
                        horizontalAlignment = Alignment.CenterHorizontally
                    ) {
                        Text("🕊 Welcome to Doot (दूत)", color = Color.White, fontSize = 20.sp, fontWeight = FontWeight.Bold)
                        Spacer(modifier = Modifier.height(10.dp))
                        Text(
                            "Fast, sovereign, zero-server P2P messenger and file transfer powered by Iroh QUIC.\n\nTap '+ New Chat' above to connect using a shareable ticket.",
                            color = Color(0xFF94A3B8),
                            fontSize = 14.sp,
                            textAlign = androidx.compose.ui.text.style.TextAlign.Center
                        )
                    }
                } else {
                    LazyColumn(
                        state = listState,
                        modifier = Modifier
                            .fillMaxSize()
                            .padding(horizontal = 12.dp, vertical = 8.dp),
                        verticalArrangement = Arrangement.spacedBy(8.dp)
                    ) {
                        items(state.messages) { msg ->
                            MessageBubble(msg, context)
                        }

                        if (state.isPeerTyping) {
                            item {
                                Text(
                                    "Typing...",
                                    color = Color(0xFF93C5FD),
                                    fontSize = 12.sp,
                                    fontStyle = androidx.compose.ui.text.font.FontStyle.Italic,
                                    modifier = Modifier.padding(start = 8.dp, top = 4.dp)
                                )
                            }
                        }
                    }
                }
            }

            // Bottom Input Bar
            if (state.activePeer != null) {
                Surface(
                    color = Color(0xFF0F172A),
                    modifier = Modifier.fillMaxWidth()
                ) {
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(horizontal = 8.dp, vertical = 6.dp),
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        // File attachment button
                        IconButton(
                            onClick = { filePickerLauncher.launch("*/*") },
                            modifier = Modifier.size(38.dp)
                        ) {
                            Text("📎", fontSize = 18.sp)
                        }

                        Spacer(modifier = Modifier.width(4.dp))

                        OutlinedTextField(
                            value = inputText,
                            onValueChange = {
                                inputText = it
                                viewModel.sendTyping(it.isNotEmpty())
                            },
                            placeholder = { Text("Write a message...", color = Color(0xFF64748B), fontSize = 14.sp) },
                            colors = OutlinedTextFieldDefaults.colors(
                                focusedTextColor = Color.White,
                                unfocusedTextColor = Color.White,
                                focusedBorderColor = Color(0xFF3B82F6),
                                unfocusedBorderColor = Color(0xFF334155),
                                focusedContainerColor = Color(0xFF1E293B),
                                unfocusedContainerColor = Color(0xFF1E293B)
                            ),
                            shape = RoundedCornerShape(20.dp),
                            modifier = Modifier.weight(1f),
                            singleLine = true
                        )

                        Spacer(modifier = Modifier.width(8.dp))

                        Button(
                            onClick = {
                                if (inputText.isNotBlank()) {
                                    viewModel.sendMessage(inputText)
                                    inputText = ""
                                }
                            },
                            shape = CircleShape,
                            colors = ButtonDefaults.buttonColors(containerColor = Color(0xFF2563EB)),
                            contentPadding = PaddingValues(12.dp)
                        ) {
                            Text("➤", fontSize = 16.sp, color = Color.White)
                        }
                    }
                }
            }
        }
    }

    // Connect Peer Modal Dialog
    if (showConnectDialog) {
        var peerTicket by remember { mutableStateOf("") }
        var peerNick by remember { mutableStateOf("Desktop PC") }

        AlertDialog(
            onDismissRequest = { showConnectDialog = false },
            title = { Text("Connect to Peer", color = Color.White, fontWeight = FontWeight.Bold) },
            text = {
                Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                    Text(
                        "Paste your Doot ticket (iroh-msg:...):",
                        color = Color(0xFFCBD5E1),
                        fontSize = 13.sp
                    )
                    OutlinedTextField(
                        value = peerTicket,
                        onValueChange = { peerTicket = it },
                        placeholder = { Text("iroh-msg:...", color = Color(0xFF64748B)) },
                        colors = OutlinedTextFieldDefaults.colors(
                            focusedTextColor = Color.White,
                            unfocusedTextColor = Color.White,
                            focusedContainerColor = Color(0xFF1E293B),
                            unfocusedContainerColor = Color(0xFF1E293B)
                        ),
                        singleLine = true,
                        modifier = Modifier.fillMaxWidth()
                    )
                    OutlinedTextField(
                        value = peerNick,
                        onValueChange = { peerNick = it },
                        placeholder = { Text("Nickname (e.g. My PC)", color = Color(0xFF64748B)) },
                        colors = OutlinedTextFieldDefaults.colors(
                            focusedTextColor = Color.White,
                            unfocusedTextColor = Color.White,
                            focusedContainerColor = Color(0xFF1E293B),
                            unfocusedContainerColor = Color(0xFF1E293B)
                        ),
                        singleLine = true,
                        modifier = Modifier.fillMaxWidth()
                    )
                }
            },
            confirmButton = {
                Button(
                    onClick = {
                        if (peerTicket.isNotBlank()) {
                            viewModel.connectPeer(peerTicket, peerNick)
                            showConnectDialog = false
                        }
                    },
                    colors = ButtonDefaults.buttonColors(containerColor = Color(0xFF2563EB))
                ) {
                    Text("Connect")
                }
            },
            dismissButton = {
                TextButton(onClick = { showConnectDialog = false }) {
                    Text("Cancel", color = Color(0xFF94A3B8))
                }
            },
            containerColor = Color(0xFF1E293B)
        )
    }
}

@Composable
fun MessageBubble(msg: MessageItem, context: Context) {
    val alignment = if (msg.isMine) Alignment.End else Alignment.Start
    val bgColor = if (msg.isMine) Color(0xFF2563EB) else Color(0xFF1E293B)

    Column(
        modifier = Modifier.fillMaxWidth(),
        horizontalAlignment = alignment
    ) {
        Surface(
            shape = RoundedCornerShape(
                topStart = 14.dp,
                topEnd = 14.dp,
                bottomStart = if (msg.isMine) 14.dp else 2.dp,
                bottomEnd = if (msg.isMine) 2.dp else 14.dp
            ),
            color = bgColor,
            modifier = Modifier.widthIn(max = 290.dp)
        ) {
            Column(modifier = Modifier.padding(horizontal = 12.dp, vertical = 8.dp)) {
                if (!msg.isMine) {
                    Text(
                        msg.senderName,
                        fontWeight = FontWeight.Bold,
                        fontSize = 11.sp,
                        color = Color(0xFF93C5FD)
                    )
                    Spacer(modifier = Modifier.height(2.dp))
                }

                // Attachment card if present
                msg.attachment?.let { att ->
                    Surface(
                        color = Color(0x33000000),
                        shape = RoundedCornerShape(8.dp),
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(bottom = 6.dp)
                            .clickable {
                                if (!att.localPath.isNullOrBlank()) {
                                    try {
                                        val file = java.io.File(att.localPath)
                                        val uri = FileProvider.getUriForFile(
                                            context,
                                            "${context.packageName}.fileprovider",
                                            file
                                        )
                                        val intent = Intent(Intent.ACTION_VIEW).apply {
                                            setDataAndType(uri, att.mimeType.ifEmpty { "*/*" })
                                            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
                                        }
                                        context.startActivity(intent)
                                    } catch (e: Exception) {
                                        Toast.makeText(context, "Cannot open file: ${e.message}", Toast.LENGTH_SHORT).show()
                                    }
                                }
                            }
                    ) {
                        Row(
                            modifier = Modifier.padding(8.dp),
                            verticalAlignment = Alignment.CenterVertically
                        ) {
                            Text(if (att.isDirectory) "📁" else "📄", fontSize = 20.sp)
                            Spacer(modifier = Modifier.width(8.dp))
                            Column(modifier = Modifier.weight(1f)) {
                                Text(
                                    att.fileName,
                                    color = Color.White,
                                    fontWeight = FontWeight.Medium,
                                    fontSize = 13.sp,
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis
                                )
                                Text(
                                    formatFileSize(att.fileSize),
                                    color = Color(0xFF94A3B8),
                                    fontSize = 11.sp
                                )
                            }
                        }
                    }
                }

                Text(msg.content, color = Color.White, fontSize = 14.sp)
                Spacer(modifier = Modifier.height(4.dp))
                Row(
                    modifier = Modifier.align(Alignment.End),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    val statusIcon = when (msg.status.lowercase()) {
                        "sending" -> "🕒"
                        "sent" -> "✓"
                        "delivered" -> "✓✓"
                        "failed" -> "⚠"
                        else -> "✓"
                    }
                    Text(
                        statusIcon,
                        fontSize = 10.sp,
                        color = Color(0xFFCBD5E1)
                    )
                }
            }
        }
    }
}

fun copyUriToCache(context: Context, uri: Uri): String? {
    return try {
        val returnCursor = context.contentResolver.query(uri, null, null, null, null) ?: return null
        val nameIndex = returnCursor.getColumnIndex(OpenableColumns.DISPLAY_NAME)
        returnCursor.moveToFirst()
        val name = returnCursor.getString(nameIndex)
        returnCursor.close()

        val file = java.io.File(context.cacheDir, name)
        context.contentResolver.openInputStream(uri)?.use { input ->
            java.io.FileOutputStream(file).use { output ->
                input.copyTo(output)
            }
        }
        file.absolutePath
    } catch (e: Exception) {
        e.printStackTrace()
        null
    }
}

fun formatFileSize(bytes: Long): String {
    if (bytes <= 0) return "0 B"
    val units = arrayOf("B", "KB", "MB", "GB", "TB")
    val digitGroups = (Math.log10(bytes.toDouble()) / Math.log10(1024.0)).toInt()
    val value = bytes / Math.pow(1024.0, digitGroups.toDouble())
    return "%.1f %s".format(value, units[digitGroups])
}
