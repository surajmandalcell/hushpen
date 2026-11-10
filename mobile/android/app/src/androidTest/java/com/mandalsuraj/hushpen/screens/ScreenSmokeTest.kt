package com.mandalsuraj.hushpen.screens

import android.os.Build
import android.view.WindowInsets
import android.webkit.WebView
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.test.espresso.Espresso.onView
import androidx.test.espresso.Espresso.pressBack
import androidx.test.espresso.assertion.ViewAssertions.matches
import androidx.test.espresso.matcher.ViewMatchers.isAssignableFrom
import androidx.test.espresso.matcher.ViewMatchers.isDisplayed
import com.mandalsuraj.hushpen.MainActivity
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test

class ScreenSmokeTest {
    @get:Rule val compose = createAndroidComposeRule<MainActivity>()

    @Test fun navigationAndSettingsRemainAvailableWithoutDownloadedModels() {
        if (Build.VERSION.SDK_INT >= 35) {
            val keyboard = compose.onNodeWithText("Keyboard").assertIsDisplayed().fetchSemanticsNode()
            val systemTop = compose.runOnIdle {
                checkNotNull(compose.activity.window.decorView.rootWindowInsets).getInsets(WindowInsets.Type.systemBars()).top
            }
            assertTrue("Home must stay below the system bars.", keyboard.boundsInWindow.top >= systemTop)
        }
        compose.onNodeWithText("Settings").performScrollTo().performClick()
        compose.onNodeWithText("No system model on this phone. Rule cleanup still works.").assertIsDisplayed()
        val haptics = compose.onNodeWithText("Haptics").performScrollTo()
        val before = haptics.fetchSemanticsNode().config[SemanticsProperties.ToggleableState]
        haptics.performClick()
        compose.waitUntil(5_000) { haptics.fetchSemanticsNode().config[SemanticsProperties.ToggleableState] != before }
        haptics.performClick()
        compose.waitUntil(5_000) { haptics.fetchSemanticsNode().config[SemanticsProperties.ToggleableState] == before }
        compose.onNodeWithText("Open-source licenses").performScrollTo().performClick()
        onView(isAssignableFrom(WebView::class.java)).check(matches(isDisplayed())).check { view, problem ->
            if (problem != null) throw problem
            if (Build.VERSION.SDK_INT >= 35) {
                val web = checkNotNull(view)
                val location = IntArray(2)
                web.getLocationOnScreen(location)
                val systemTop = checkNotNull(web.rootWindowInsets).getInsets(WindowInsets.Type.systemBars()).top
                assertTrue("Licenses must stay below the system bars.", location[1] >= systemTop)
            }
        }
        pressBack()
        compose.onNodeWithContentDescription("Back").performClick()
        compose.onNodeWithText("Devices").performScrollTo().performClick()
        compose.onNodeWithText("No paired desktops.").performScrollTo().assertIsDisplayed()
        pressBack()
        compose.onNodeWithText("Models").performScrollTo().performClick()
        compose.onNodeWithText("Speech to Text").assertIsDisplayed()
        compose.onNodeWithContentDescription("Refresh").assertIsDisplayed()
        compose.waitUntil(15_000) {
            compose.onAllNodesWithText("Parakeet", substring = true).fetchSemanticsNodes().isNotEmpty()
        }
    }
}
