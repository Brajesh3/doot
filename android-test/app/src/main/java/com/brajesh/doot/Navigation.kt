package com.brajesh.doot

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ChatBubble
import androidx.compose.material.icons.filled.Info
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material.icons.outlined.ChatBubbleOutline
import androidx.compose.material.icons.outlined.Info
import androidx.compose.material.icons.outlined.Settings
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import com.brajesh.doot.messenger.MessengerViewModel
import com.brajesh.doot.ui.about.AboutScreen
import com.brajesh.doot.ui.chat.ChatScreen
import com.brajesh.doot.ui.home.HomeScreen
import com.brajesh.doot.ui.settings.SettingsScreen

enum class DootNavTab(
    val title: String,
    val selectedIcon: ImageVector,
    val unselectedIcon: ImageVector
) {
    CHATS("Chats", Icons.Filled.ChatBubble, Icons.Outlined.ChatBubbleOutline),
    SETTINGS("Settings", Icons.Filled.Settings, Icons.Outlined.Settings),
    ABOUT("About", Icons.Filled.Info, Icons.Outlined.Info)
}

enum class DootScreen {
    HOME,
    CHAT,
    SETTINGS,
    ABOUT
}

@Composable
fun DootApp(
    viewModel: MessengerViewModel,
    modifier: Modifier = Modifier
) {
    val state by viewModel.uiState.collectAsState()
    var currentScreen by remember { mutableStateOf(DootScreen.HOME) }
    var currentTab by remember { mutableStateOf(DootNavTab.CHATS) }

    val showBottomBar = currentScreen != DootScreen.CHAT

    Scaffold(
        modifier = modifier.fillMaxSize(),
        contentWindowInsets = WindowInsets(0, 0, 0, 0),
        bottomBar = {
            if (showBottomBar) {
                NavigationBar {
                    DootNavTab.entries.forEach { tab ->
                        val isSelected = currentTab == tab && (
                            (tab == DootNavTab.CHATS && currentScreen == DootScreen.HOME) ||
                            (tab == DootNavTab.SETTINGS && currentScreen == DootScreen.SETTINGS) ||
                            (tab == DootNavTab.ABOUT && currentScreen == DootScreen.ABOUT)
                        )
                        NavigationBarItem(
                            selected = isSelected,
                            onClick = {
                                currentTab = tab
                                currentScreen = when (tab) {
                                    DootNavTab.CHATS -> DootScreen.HOME
                                    DootNavTab.SETTINGS -> DootScreen.SETTINGS
                                    DootNavTab.ABOUT -> DootScreen.ABOUT
                                }
                            },
                            icon = {
                                val totalUnread = state.contacts.sumOf { it.unreadCount }
                                if (tab == DootNavTab.CHATS && totalUnread > 0) {
                                    BadgedBox(
                                        badge = {
                                            Badge { Text("$totalUnread") }
                                        }
                                    ) {
                                        Icon(
                                            imageVector = if (isSelected) tab.selectedIcon else tab.unselectedIcon,
                                            contentDescription = tab.title
                                        )
                                    }
                                } else {
                                    Icon(
                                        imageVector = if (isSelected) tab.selectedIcon else tab.unselectedIcon,
                                        contentDescription = tab.title
                                    )
                                }
                            },
                            label = { Text(tab.title) }
                        )
                    }
                }
            }
        }
    ) { innerPadding ->
        AnimatedContent(
            targetState = currentScreen,
            transitionSpec = {
                if (targetState == DootScreen.CHAT) {
                    slideInHorizontally { width -> width } + fadeIn() togetherWith
                            slideOutHorizontally { width -> -width } + fadeOut()
                } else if (initialState == DootScreen.CHAT) {
                    slideInHorizontally { width -> -width } + fadeIn() togetherWith
                            slideOutHorizontally { width -> width } + fadeOut()
                } else {
                    fadeIn() togetherWith fadeOut()
                }
            },
            label = "screen_transition",
            modifier = Modifier
                .fillMaxSize()
                .padding(bottom = innerPadding.calculateBottomPadding())
        ) { screen ->
            when (screen) {
                DootScreen.HOME -> {
                    HomeScreen(
                        viewModel = viewModel,
                        onOpenChat = { contact ->
                            viewModel.selectPeer(contact)
                            currentScreen = DootScreen.CHAT
                        }
                    )
                }
                DootScreen.CHAT -> {
                    ChatScreen(
                        viewModel = viewModel,
                        onBack = {
                            currentScreen = DootScreen.HOME
                            currentTab = DootNavTab.CHATS
                        }
                    )
                }
                DootScreen.SETTINGS -> {
                    SettingsScreen(
                        viewModel = viewModel,
                        onNavigateToAbout = {
                            currentScreen = DootScreen.ABOUT
                            currentTab = DootNavTab.ABOUT
                        }
                    )
                }
                DootScreen.ABOUT -> {
                    AboutScreen(
                        onBack = {
                            currentScreen = DootScreen.SETTINGS
                            currentTab = DootNavTab.SETTINGS
                        }
                    )
                }
            }
        }
    }
}
