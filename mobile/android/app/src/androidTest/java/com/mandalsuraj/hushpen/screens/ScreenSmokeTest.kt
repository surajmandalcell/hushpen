package com.mandalsuraj.hushpen.screens

import android.os.Build
import android.view.WindowInsets
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import com.mandalsuraj.hushpen.MainActivity
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test

class ScreenSmokeTest {
    @get:Rule val compose = createAndroidComposeRule<MainActivity>()

    @Test fun navigationAndSettingsRemainAvailableWithoutDownloadedModels() {
        if (Build.VERSION.SDK_INT >= 35) {
            val onboarding = compose.onNodeWithText("Onboarding").assertIsDisplayed().fetchSemanticsNode()
            val systemTop = compose.runOnIdle {
                checkNotNull(compose.activity.window.decorView.rootWindowInsets).getInsets(WindowInsets.Type.systemBars()).top
            }
            assertTrue("Onboarding must stay below the system bars.", onboarding.boundsInWindow.top >= systemTop)
        }
        compose.onNodeWithText("Settings", useUnmergedTree = true).performScrollTo().performClick()
        compose.onNodeWithText("System cleanup available: false").assertIsDisplayed()
        compose.onNodeWithContentDescription("Keyboard haptics").performScrollTo().performClick()
        compose.onNodeWithText("Devices", useUnmergedTree = true).performScrollTo().performClick()
        compose.onNodeWithText("No paired desktops.").performScrollTo().assertIsDisplayed()
        compose.onNodeWithText("Models", useUnmergedTree = true).performScrollTo().performClick()
        compose.onNodeWithText("Refresh models").performScrollTo().assertIsDisplayed()
    }
}
