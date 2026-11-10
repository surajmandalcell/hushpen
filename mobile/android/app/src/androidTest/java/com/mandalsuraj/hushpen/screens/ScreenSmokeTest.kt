package com.mandalsuraj.hushpen.screens

import android.os.Build
import android.view.WindowInsets
import android.webkit.WebView
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createAndroidComposeRule
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
            val onboarding = compose.onNodeWithText("Onboarding").assertIsDisplayed().fetchSemanticsNode()
            val systemTop = compose.runOnIdle {
                checkNotNull(compose.activity.window.decorView.rootWindowInsets).getInsets(WindowInsets.Type.systemBars()).top
            }
            assertTrue("Onboarding must stay below the system bars.", onboarding.boundsInWindow.top >= systemTop)
        }
        compose.onNodeWithText("Settings", useUnmergedTree = true).performScrollTo().performClick()
        compose.onNodeWithText("System cleanup available: false").assertIsDisplayed()
        compose.onNodeWithContentDescription("Keyboard haptics").performScrollTo().performClick()
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
        compose.onNodeWithText("Devices", useUnmergedTree = true).performScrollTo().performClick()
        compose.onNodeWithText("No paired desktops.").performScrollTo().assertIsDisplayed()
        compose.onNodeWithText("Models", useUnmergedTree = true).performScrollTo().performClick()
        compose.onNodeWithText("Refresh models").performScrollTo().assertIsDisplayed()
    }
}
