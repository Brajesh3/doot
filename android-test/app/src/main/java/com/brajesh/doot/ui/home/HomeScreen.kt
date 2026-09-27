package com.brajesh.doot.ui.home

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.widget.Toast
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.outlined.ChatBubbleOutline
import androidx.compose.material.icons.outlined.ContentCopy
import androidx.compose.material.icons.outlined.Person
import androidx.compose.material.icons.outlined.QrCode
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
import com.brajesh.doot.messenger.ContactItem
import com.brajesh.doot.messenger.MessengerUiState
import com.brajesh.doot.messenger.MessengerViewModel

import com.brajesh.doot.ui.components.expressivePressScale
import androidx.compose.foundation.interaction.MutableInteractionSource

@OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun HomeScreen(
    viewModel: MessengerViewModel,
    onOpenChat: (ContactItem) -> Unit,
    modifier: Modifier = Modifier
) {
    val state by viewModel.uiState.collectAsState()
    val context = LocalContext.current
    var showNewChatDialog by remember { mutableStateOf(false) }
    var showIdentitySheet by remember { mutableStateOf(false) }
    var showFabMenu by remember { mutableStateOf(false) }

    Scaffold(
        modifier = modifier.fillMaxSize(),
        contentWindowInsets = WindowInsets(0, 0, 0, 0),
        topBar = {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .statusBarsPadding()
                    .padding(horizontal = 16.dp, vertical = 6.dp)
            ) {
                // Expressive Floating Search Bar (Google Drive / Gmail style)
                Surface(
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(56.dp),
                    shape = RoundedCornerShape(28.dp),
                    color = MaterialTheme.colorScheme.surfaceContainerHigh,
                    tonalElevation = 4.dp
                ) {
                    Row(
                        modifier = Modifier
                            .fillMaxSize()
                            .padding(horizontal = 16.dp),
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Icon(
                            imageVector = Icons.Default.Search,
                            contentDescription = "Search",
                            tint = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                        Spacer(modifier = Modifier.width(12.dp))
                        TextField(
                            value = state.searchQuery,
                            onValueChange = { viewModel.updateSearchQuery(it) },
                            placeholder = {
                                Text(
                                    "Search conversations...",
                                    style = MaterialTheme.typography.bodyLarge,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant
                                )
                            },
                            singleLine = true,
                            colors = TextFieldDefaults.colors(
                                focusedContainerColor = Color.Transparent,
                                unfocusedContainerColor = Color.Transparent,
                                focusedIndicatorColor = Color.Transparent,
                                unfocusedIndicatorColor = Color.Transparent
                            ),
                            modifier = Modifier.weight(1f)
                        )
                        if (state.searchQuery.isNotEmpty()) {
                            IconButton(onClick = { viewModel.updateSearchQuery("") }) {
                                Icon(
                                    imageVector = Icons.Default.Close,
                                    contentDescription = "Clear search",
                                    tint = MaterialTheme.colorScheme.onSurfaceVariant
                                )
                            }
                        }

                        // Avatar on top-right opening Identity Modal
                        Box(
                            modifier = Modifier
                                .size(36.dp)
                                .clip(CircleShape)
                                .background(MaterialTheme.colorScheme.primaryContainer)
                                .clickable { showIdentitySheet = true },
                            contentAlignment = Alignment.Center
                        ) {
                            Text(
                                text = "🕊",
                                fontSize = 18.sp
                            )
                        }
                    }
                }

                // Banner alert if present
                AnimatedVisibility(
                    visible = state.bannerMessage != null,
                    enter = fadeIn(animationSpec = MaterialTheme.motionScheme.fastEffectsSpec()),
                    exit = fadeOut(animationSpec = MaterialTheme.motionScheme.fastEffectsSpec())
                ) {

                    state.bannerMessage?.let { banner ->
                        Card(
                            modifier = Modifier
                                .fillMaxWidth()
                                .padding(top = 8.dp),
                            shape = RoundedCornerShape(12.dp),
                            colors = CardDefaults.cardColors(
                                containerColor = MaterialTheme.colorScheme.secondaryContainer
                            )
                        ) {
                            Row(
                                modifier = Modifier
                                    .fillMaxWidth()
                                    .padding(horizontal = 14.dp, vertical = 8.dp),
                                horizontalArrangement = Arrangement.SpaceBetween,
                                verticalAlignment = Alignment.CenterVertically
                            ) {
                                Text(
                                    text = banner,
                                    style = MaterialTheme.typography.bodyMedium,
                                    color = MaterialTheme.colorScheme.onSecondaryContainer,
                                    modifier = Modifier.weight(1f)
                                )
                                IconButton(
                                    onClick = { viewModel.dismissBanner() },
                                    modifier = Modifier.size(24.dp)
                                ) {
                                    Icon(
                                        imageVector = Icons.Default.Close,
                                        contentDescription = "Dismiss",
                                        tint = MaterialTheme.colorScheme.onSecondaryContainer
                                    )
                                }
                            }
                        }
                    }
                }
            }
        },
        floatingActionButton = {
            FloatingActionButtonMenu(
                expanded = showFabMenu,
                button = {
                    ToggleFloatingActionButton(
                        checked = showFabMenu,
                        onCheckedChange = { showFabMenu = !showFabMenu }
                    ) {
                        Icon(
                            imageVector = if (showFabMenu) Icons.Default.Close else Icons.Default.Add,
                            contentDescription = if (showFabMenu) "Close menu" else "New chat options"
                        )
                    }
                }
            ) {
                FloatingActionButtonMenuItem(
                    onClick = {
                        showFabMenu = false
                        showNewChatDialog = true
                    },
                    text = { Text("Connect Peer") },
                    icon = { Icon(Icons.Outlined.Person, contentDescription = null) }
                )
                FloatingActionButtonMenuItem(
                    onClick = {
                        showFabMenu = false
                        showIdentitySheet = true
                    },
                    text = { Text("My Sovereign Ticket") },
                    icon = { Icon(Icons.Outlined.ContentCopy, contentDescription = null) }
                )
            }
        }
    ) { innerPadding ->
        val contacts = state.filteredContacts

        if (contacts.isEmpty()) {
            Box(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(innerPadding)
                    .padding(24.dp),
                contentAlignment = Alignment.Center
            ) {
                Card(
                    modifier = Modifier.fillMaxWidth(),
                    shape = RoundedCornerShape(24.dp),
                    colors = CardDefaults.cardColors(
                        containerColor = MaterialTheme.colorScheme.surfaceContainer
                    )
                ) {
                    Column(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(32.dp),
                        horizontalAlignment = Alignment.CenterHorizontally
                    ) {
                        Box(
                            modifier = Modifier
                                .size(64.dp)
                                .clip(CircleShape)
                                .background(MaterialTheme.colorScheme.primaryContainer),
                            contentAlignment = Alignment.Center
                        ) {
                            Icon(
                                imageVector = Icons.Outlined.ChatBubbleOutline,
                                contentDescription = null,
                                tint = MaterialTheme.colorScheme.primary,
                                modifier = Modifier.size(32.dp)
                            )
                        }
                        Spacer(modifier = Modifier.height(16.dp))
                        Text(
                            text = "No Conversations Yet",
                            style = MaterialTheme.typography.titleLarge,
                            fontWeight = FontWeight.SemiBold
                        )
                        Spacer(modifier = Modifier.height(8.dp))
                        Text(
                            text = "Connect directly with another peer over encrypted QUIC without servers.",
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            textAlign = androidx.compose.ui.text.style.TextAlign.Center
                        )
                        Spacer(modifier = Modifier.height(20.dp))
                        Button(
                            onClick = { showNewChatDialog = true },
                            shape = RoundedCornerShape(12.dp)
                        ) {
                            Text("Connect with a Peer")
                        }
                    }
                }
            }
        } else {
            LazyColumn(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(innerPadding)
                    .padding(horizontal = 12.dp),
                contentPadding = PaddingValues(vertical = 8.dp),
                verticalArrangement = Arrangement.spacedBy(4.dp)
            ) {
                items(contacts, key = { it.peerKey }) { contact ->
                    val connInfo = state.connectionTypes[contact.peerKey]
                    ConversationItemCard(
                        contact = contact,
                        connInfo = connInfo,
                        onClick = { onOpenChat(contact) }
                    )
                }
            }
        }
    }

    if (showNewChatDialog) {
        ConnectPeerDialog(
            isConnecting = state.isConnecting,
            onDismiss = { showNewChatDialog = false },
            onConnect = { ticket, nick ->
                viewModel.connectPeer(ticket, nick) { success ->
                    if (success) showNewChatDialog = false
                }
            }
        )
    }

    if (showIdentitySheet) {
        ModalBottomSheet(
            onDismissRequest = { showIdentitySheet = false },
            sheetState = rememberModalBottomSheetState()
        ) {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 24.dp)
                    .padding(bottom = 32.dp)
            ) {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    modifier = Modifier.fillMaxWidth()
                ) {
                    Box(
                        modifier = Modifier
                            .size(52.dp)
                            .clip(CircleShape)
                            .background(MaterialTheme.colorScheme.primaryContainer),
                        contentAlignment = Alignment.Center
                    ) {
                        Text("🕊", fontSize = 24.sp)
                    }
                    Spacer(modifier = Modifier.width(16.dp))
                    Column {
                        Text(
                            text = state.myNickname,
                            style = MaterialTheme.typography.titleLarge,
                            fontWeight = FontWeight.Bold
                        )
                        val shortId = if (state.myNodeId.length > 12) state.myNodeId.take(12) + "..." else state.myNodeId
                        Text(
                            text = "Node ID: $shortId",
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                    }
                }

                Spacer(modifier = Modifier.height(20.dp))
                HorizontalDivider()
                Spacer(modifier = Modifier.height(16.dp))

                Text(
                    text = "Shareable Ticket (doot:...)",
                    style = MaterialTheme.typography.titleMedium,
                    fontWeight = FontWeight.SemiBold
                )
                Spacer(modifier = Modifier.height(4.dp))
                Text(
                    text = "Give this ticket to another peer so they can connect with you directly over QUIC.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )

                Spacer(modifier = Modifier.height(12.dp))
                Surface(
                    shape = RoundedCornerShape(12.dp),
                    color = MaterialTheme.colorScheme.surfaceContainerHigh,
                    modifier = Modifier.fillMaxWidth()
                ) {
                    Text(
                        text = state.myTicket.ifEmpty { "Generating cryptographic ticket..." },
                        style = MaterialTheme.typography.bodySmall,
                        modifier = Modifier.padding(12.dp),
                        maxLines = 3,
                        overflow = TextOverflow.Ellipsis
                    )
                }

                Spacer(modifier = Modifier.height(16.dp))
                FilledTonalButton(
                    onClick = {
                        val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
                        val clip = ClipData.newPlainText("Doot Ticket", state.myTicket)
                        clipboard.setPrimaryClip(clip)
                        Toast.makeText(context, "Ticket copied to clipboard!", Toast.LENGTH_SHORT).show()
                    },
                    modifier = Modifier.fillMaxWidth(),
                    shape = RoundedCornerShape(12.dp)
                ) {
                    Icon(Icons.Outlined.ContentCopy, contentDescription = null)
                    Spacer(modifier = Modifier.width(8.dp))
                    Text("Copy Shareable Ticket")
                }
            }
        }
    }
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun ConversationItemCard(
    contact: ContactItem,
    connInfo: com.brajesh.doot.messenger.ConnectionInfo?,
    onClick: () -> Unit
) {
    val interactionSource = remember { MutableInteractionSource() }
    Surface(
        onClick = onClick,
        shape = RoundedCornerShape(16.dp),
        color = Color.Transparent,
        interactionSource = interactionSource,
        modifier = Modifier
            .fillMaxWidth()
            .expressivePressScale(interactionSource, pressedScale = 0.97f)
    ) {

        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 8.dp, vertical = 10.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            // Peer Avatar with telemetry status dot
            Box(contentAlignment = Alignment.BottomEnd) {
                Box(
                    modifier = Modifier
                        .size(50.dp)
                        .clip(CircleShape)
                        .background(MaterialTheme.colorScheme.primaryContainer),
                    contentAlignment = Alignment.Center
                ) {
                    val initial = contact.nickname.firstOrNull()?.uppercase() ?: "P"
                    Text(
                        text = initial,
                        style = MaterialTheme.typography.titleMedium,
                        fontWeight = FontWeight.Bold,
                        color = MaterialTheme.colorScheme.onPrimaryContainer
                    )
                }

                // Dot indicator
                val dotColor = when {
                    connInfo?.isDirect() == true -> Color(0xFF1E8E3E)
                    connInfo?.isRelayed() == true -> Color(0xFFE37400)
                    else -> Color.Gray
                }
                Box(
                    modifier = Modifier
                        .size(14.dp)
                        .clip(CircleShape)
                        .background(MaterialTheme.colorScheme.surface)
                        .padding(2.dp)
                ) {
                    Box(
                        modifier = Modifier
                            .fillMaxSize()
                            .clip(CircleShape)
                            .background(dotColor)
                    )
                }
            }

            Spacer(modifier = Modifier.width(14.dp))

            Column(modifier = Modifier.weight(1f)) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Text(
                        text = contact.nickname,
                        style = MaterialTheme.typography.titleMedium,
                        fontWeight = FontWeight.SemiBold,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis
                    )
                    connInfo?.let { info ->
                        Surface(
                            shape = RoundedCornerShape(8.dp),
                            color = if (info.isDirect()) Color(0xFFE6F4EA) else Color(0xFFFEF7E0)
                        ) {
                            Text(
                                text = info.displayLabel(),
                                style = MaterialTheme.typography.labelSmall,
                                color = if (info.isDirect()) Color(0xFF137333) else Color(0xFFB06000),
                                modifier = Modifier.padding(horizontal = 6.dp, vertical = 2.dp)
                            )
                        }
                    }
                }

                Spacer(modifier = Modifier.height(2.dp))
                val shortPk = if (contact.peerKey.length > 16) contact.peerKey.take(16) + "..." else contact.peerKey
                Text(
                    text = shortPk,
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis
                )
            }

            if (contact.unreadCount > 0) {
                Spacer(modifier = Modifier.width(8.dp))
                Badge(
                    containerColor = MaterialTheme.colorScheme.primary,
                    contentColor = MaterialTheme.colorScheme.onPrimary
                ) {
                    Text("${contact.unreadCount}")
                }
            }
        }
    }
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun ConnectPeerDialog(
    isConnecting: Boolean,
    onDismiss: () -> Unit,
    onConnect: (String, String) -> Unit
) {
    var ticketText by remember { mutableStateOf("") }
    var nicknameText by remember { mutableStateOf("") }

    AlertDialog(
        onDismissRequest = { if (!isConnecting) onDismiss() },
        title = {
            Text(
                "Connect to a Peer",
                style = MaterialTheme.typography.titleLarge,
                fontWeight = FontWeight.SemiBold
            )
        },
        text = {
            Column(modifier = Modifier.fillMaxWidth()) {
                Text(
                    "Paste peer's shareable ticket (doot:...) or public key:",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
                Spacer(modifier = Modifier.height(8.dp))
                OutlinedTextField(
                    value = ticketText,
                    onValueChange = { ticketText = it },
                    placeholder = { Text("doot:... or hex key") },
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(90.dp),
                    shape = RoundedCornerShape(12.dp)
                )

                Spacer(modifier = Modifier.height(14.dp))
                Text(
                    "Friendly Nickname (Optional):",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
                Spacer(modifier = Modifier.height(6.dp))
                OutlinedTextField(
                    value = nicknameText,
                    onValueChange = { nicknameText = it },
                    placeholder = { Text("e.g. Alice") },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth(),
                    shape = RoundedCornerShape(12.dp)
                )
            }
        },
        confirmButton = {
            Button(
                onClick = { onConnect(ticketText, nicknameText) },
                enabled = ticketText.isNotBlank() && !isConnecting,
                shape = RoundedCornerShape(12.dp)
            ) {
                if (isConnecting) {
                    LoadingIndicator(
                        modifier = Modifier.size(18.dp),
                        color = MaterialTheme.colorScheme.onPrimary
                    )
                    Spacer(modifier = Modifier.width(8.dp))
                    Text("Connecting...")
                } else {
                    Text("Connect & Chat")
                }
            }
        },
        dismissButton = {
            TextButton(
                onClick = onDismiss,
                enabled = !isConnecting
            ) {
                Text("Cancel")
            }
        }
    )
}
