package com.mandalsuraj.hushpen.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.windowInsetsTopHeight
import androidx.compose.foundation.layout.calculateEndPadding
import androidx.compose.foundation.layout.calculateStartPadding
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LargeTopAppBar
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.key
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.platform.LocalLayoutDirection
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.foundation.selection.toggleable
import com.mandalsuraj.hushpen.R

/** A screen of grouped lists. Home has a plain large title; other screens get a back row and a title that collapses. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ListScreen(
    title: String,
    onBack: (() -> Unit)? = null,
    actions: @Composable RowScope.() -> Unit = {},
    floatingActionButton: @Composable () -> Unit = {},
    content: @Composable ColumnScope.() -> Unit,
) {
    val surfaces = LocalSurfaces.current
    val scroll = TopAppBarDefaults.exitUntilCollapsedScrollBehavior()
    Scaffold(
        modifier = Modifier.fillMaxSize().then(if (onBack != null) Modifier.nestedScroll(scroll.nestedScrollConnection) else Modifier),
        containerColor = surfaces.page,
        topBar = {
            if (onBack == null) {
                Spacer(Modifier.fillMaxWidth().windowInsetsTopHeight(WindowInsets.statusBars).background(surfaces.page))
            } else {
                val type = MaterialTheme.typography
                MaterialTheme(typography = type.copy(headlineMedium = type.headlineMedium.copy(fontSize = 30.sp, lineHeight = 38.sp))) {
                    LargeTopAppBar(
                        title = { Text(title, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                        navigationIcon = {
                            IconButton(onClick = onBack) {
                                Icon(painterResource(R.drawable.ms_arrow_back), contentDescription = "Back")
                            }
                        },
                        actions = actions,
                        expandedHeight = 128.dp,
                        colors = TopAppBarDefaults.topAppBarColors(containerColor = surfaces.page, scrolledContainerColor = surfaces.page),
                        scrollBehavior = scroll,
                    )
                }
            }
        },
        floatingActionButton = floatingActionButton,
    ) { padding ->
        val direction = LocalLayoutDirection.current
        Column(
            Modifier
                .fillMaxSize()
                .padding(
                    top = padding.calculateTopPadding(),
                    start = padding.calculateStartPadding(direction),
                    end = padding.calculateEndPadding(direction),
                )
                .verticalScroll(rememberScrollState())
                // Room for the floating action button over the last row.
                .padding(bottom = padding.calculateBottomPadding() + 96.dp),
        ) {
            if (onBack == null) {
                Text(
                    title,
                    style = MaterialTheme.typography.displaySmall,
                    color = MaterialTheme.colorScheme.onSurface,
                    modifier = Modifier.padding(start = 24.dp, top = 26.dp, end = 24.dp, bottom = 18.dp).semantics { heading() },
                )
            }
            content()
        }
    }
}
class GroupScope internal constructor() {
    internal val rows = mutableListOf<@Composable (Shape) -> Unit>()

    fun row(content: @Composable (Shape) -> Unit) {
        rows += content
    }
}

/** Rows of one group: large outer corners, small inner corners, and 2 dp between rows. */
@Composable
fun ListGroup(modifier: Modifier = Modifier, build: GroupScope.() -> Unit) {
    val rows = GroupScope().apply(build).rows
    Column(
        modifier.fillMaxWidth().padding(horizontal = 16.dp),
        verticalArrangement = Arrangement.spacedBy(2.dp),
    ) {
        rows.forEachIndexed { index, row -> key(index) { row(groupShape(index, rows.size)) } }
    }
}

fun groupShape(index: Int, count: Int): Shape {
    val top = if (index == 0) 24.dp else 4.dp
    val bottom = if (index == count - 1) 24.dp else 4.dp
    return RoundedCornerShape(topStart = top, topEnd = top, bottomStart = bottom, bottomEnd = bottom)
}

@Composable
fun SectionHeader(text: String) {
    Text(
        text,
        style = MaterialTheme.typography.titleSmall,
        color = MaterialTheme.colorScheme.primary,
        modifier = Modifier.padding(start = 32.dp, end = 32.dp, top = 24.dp, bottom = 8.dp).semantics { heading() },
    )
}

@Composable
fun GroupFooter(text: String) {
    Text(
        text,
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.padding(start = 32.dp, end = 32.dp, top = 8.dp),
    )
}

@Composable
fun GroupItem(
    shape: Shape,
    headline: String,
    modifier: Modifier = Modifier,
    supporting: String? = null,
    supportingColor: Color = Color.Unspecified,
    supportingLines: Int = 1,
    icon: Int? = null,
    below: (@Composable () -> Unit)? = null,
    trailing: (@Composable () -> Unit)? = null,
) {
    ListItem(
        headlineContent = { Text(headline) },
        supportingContent = if (supporting == null && below == null) null else {
            {
                Column {
                    // A line break separates facts, so each fact keeps its own line instead of
                    // a long first fact pushing the second one out.
                    val lines = supporting?.split('\n').orEmpty()
                    lines.forEach { line ->
                        Text(line, color = supportingColor, maxLines = if (lines.size > 1) 1 else supportingLines, overflow = TextOverflow.Ellipsis)
                    }
                    below?.invoke()
                }
            }
        },
        leadingContent = icon?.let { { Icon(painterResource(it), contentDescription = null) } },
        trailingContent = trailing,
        colors = ListItemDefaults.colors(containerColor = LocalSurfaces.current.item),
        modifier = Modifier.clip(shape).then(modifier),
    )
}

@Composable
fun ToggleItem(
    shape: Shape,
    headline: String,
    checked: Boolean,
    onCheckedChange: (Boolean) -> Unit,
    supporting: String? = null,
    supportingLines: Int = 1,
    icon: Int? = null,
    enabled: Boolean = true,
) {
    GroupItem(
        shape = shape,
        headline = headline,
        supporting = supporting,
        supportingLines = supportingLines,
        icon = icon,
        modifier = Modifier.toggleable(checked, enabled = enabled, role = Role.Switch, onValueChange = onCheckedChange),
        trailing = { HushpenSwitch(checked, onCheckedChange = null, enabled = enabled) },
    )
}

@Composable
fun HushpenSwitch(checked: Boolean, onCheckedChange: ((Boolean) -> Unit)?, enabled: Boolean = true) {
    Switch(
        checked = checked,
        onCheckedChange = onCheckedChange,
        enabled = enabled,
        thumbContent = if (checked) {
            { Icon(painterResource(R.drawable.ms_check), contentDescription = null, modifier = Modifier.padding(2.dp)) }
        } else null,
    )
}

@Composable
fun ReadyMark(description: String = "Ready") {
    Icon(
        painterResource(R.drawable.ms_check_circle),
        contentDescription = description,
        tint = MaterialTheme.colorScheme.primary,
    )
}

@Composable
fun Notices(messages: List<String>) {
    messages.map(::readable).distinct().forEach { message ->
        NoticeCard(message, Modifier.padding(bottom = 8.dp))
    }
}

@Composable
fun NoticeCard(text: String, modifier: Modifier = Modifier) {
    val ok = text.endsWith(" paired.")
    Surface(
        color = if (ok) MaterialTheme.colorScheme.secondaryContainer else MaterialTheme.colorScheme.errorContainer,
        contentColor = if (ok) MaterialTheme.colorScheme.onSecondaryContainer else MaterialTheme.colorScheme.onErrorContainer,
        shape = RoundedCornerShape(24.dp),
        modifier = modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp)
            .semantics { liveRegion = LiveRegionMode.Polite },
    ) {
        Row(
            Modifier.padding(horizontal = 20.dp, vertical = 16.dp),
            horizontalArrangement = Arrangement.spacedBy(16.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Icon(painterResource(if (ok) R.drawable.ms_check_circle else R.drawable.ms_error), contentDescription = null)
            Text(text, style = MaterialTheme.typography.bodyMedium)
        }
    }
}

/** Rust errors arrive as `detail=<text>`; show them as a plain sentence. */
fun readable(message: String): String {
    val text = message.removePrefix("detail=").trim().replaceFirstChar { it.uppercase() }
    return if (text.endsWith('.') || text.endsWith('!') || text.endsWith('?')) text else "$text."
}
