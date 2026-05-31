use maud::{html, Markup, PreEscaped, DOCTYPE};

/// Content hash from `build.rs`, appended as `?v=...` to asset URLs for cache-busting.
pub const ASSETS_V: &str = env!("ASSETS_VERSION");

pub struct Ctx<'a> {
    pub base: &'a str,
    pub csrf: &'a str,
}

pub fn page(ctx: &Ctx<'_>, title: &str, nav: &str, content: Markup) -> Markup {
    let base = ctx.base;
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                meta name="csrf" content=(ctx.csrf);
                title { "autopulse · " (title) }
                link rel="icon" type="image/webp" href={ (base) "/ui/static/logo.webp" };
                link rel="stylesheet" href={ (base) "/ui/static/app.css?v=" (ASSETS_V) };
                script src={ (base) "/ui/static/htmx.min.js?v=" (ASSETS_V) } defer {}
                script src={ (base) "/ui/static/htmx-sse.js?v=" (ASSETS_V) } defer {}
            }
            body {
                .shell {
                    // Mobile-only drawer toggle; the rail is off-canvas below
                    // the breakpoint and slides in when this is pressed.
                    button.rail-toggle #rail-toggle type="button"
                        aria-label="Toggle navigation"
                        aria-controls="rail" aria-expanded="false"
                    {
                        (icon(icondata::LuMenu, 20))
                    }
                    .rail-backdrop #rail-backdrop {}
                    aside.rail #rail {
                        a.rail__brand href={ (base) "/ui/events" } {
                            img.rail__logo src={ (base) "/ui/static/logo.webp" } alt="autopulse";
                            .rail__brand-text {
                                span.rail__brand-name { "autopulse" }
                                // same version string the `/` route reports
                                // (git describe --always --tags; includes the `v`)
                                span.rail__brand-sub { (env!("GIT_REVISION")) }
                            }
                        }
                        nav.rail__nav {
                            (rail_link(base, "/ui/events", "Events",   icondata::LuList,            nav == "events"))
                            (rail_link(base, "/ui/add",    "Add scan", icondata::LuPlus,            nav == "add"))
                            (rail_link(base, "/ui/config", "Config",   icondata::LuSlidersHorizontal, nav == "config"))
                        }
                        .rail__spacer {}
                        .rail__foot {
                            // dot + label reflect live SSE bus state; the
                            // client script in <head> toggles these on the
                            // htmx sse lifecycle events.
                            .rail__status #bus-status {
                                span.rail__status-dot #bus-dot {}
                                span.rail__status-label #bus-label { "Idle" }
                            }
                            form.rail__logout method="post" action={ (base) "/ui/logout" } {
                                input type="hidden" name="csrf" value=(ctx.csrf);
                                button type="submit" {
                                    (icon(icondata::LuLogOut, 15))
                                    span { "Sign out" }
                                }
                            }
                        }
                    }
                    main.main {
                        .main__inner { (content) }
                    }
                }
                (bus_status_script())
                (num_format_script())
                (rail_drawer_script())
            }
        }
    }
}

/// Formats `[data-num]` elements with locale thousands separators; idempotent across HTMX swaps.
fn num_format_script() -> Markup {
    PreEscaped(
        r#"<script>
(function () {
  function fmt() {
    document.querySelectorAll('[data-num]').forEach(function (el) {
      var n = Number(el.getAttribute('data-num'));
      if (!Number.isNaN(n)) el.textContent = n.toLocaleString();
    });
    document.querySelectorAll('time.local-ts[datetime]').forEach(function (el) {
      var d = new Date(el.getAttribute('datetime'));
      if (!isNaN(d.getTime())) el.textContent = d.toLocaleString();
    });
  }
  if (document.readyState !== 'loading') fmt();
  else document.addEventListener('DOMContentLoaded', fmt);
  document.body.addEventListener('htmx:afterSwap', fmt);
})();
</script>"#
            .to_string(),
    )
}

/// Drives the rail Live indicator from HTMX SSE lifecycle events.
/// Pages without the events table stay Idle.
fn bus_status_script() -> Markup {
    PreEscaped(r#"<script>
(function () {
  function set(state) {
    var dot = document.getElementById('bus-dot');
    var label = document.getElementById('bus-label');
    if (!dot || !label) return;
    dot.className = 'rail__status-dot';
    if (state === 'live') { dot.classList.add('rail__status-dot--live'); label.textContent = 'Live'; }
    else if (state === 'error') { dot.classList.add('rail__status-dot--error'); label.textContent = 'Offline'; }
    else { label.textContent = 'Idle'; }
  }
  // Filter switches re-render the events section, which tears down the
  // old SSE connection (close) and immediately reconnects (open). Debounce
  // the offline state so that transient close-then-reopen doesn't flash
  // "Offline"; a real disconnect (no reopen) still shows it after the delay.
  var offlineTimer = null;
  function scheduleOffline() {
    clearTimeout(offlineTimer);
    offlineTimer = setTimeout(function () { set('error'); }, 1500);
  }
  document.body.addEventListener('htmx:sseOpen', function () { clearTimeout(offlineTimer); set('live'); });
  document.body.addEventListener('htmx:sseError', scheduleOffline);
  document.body.addEventListener('htmx:sseClose', scheduleOffline);
})();
</script>"#.to_string())
}

fn rail_drawer_script() -> Markup {
    PreEscaped(
        r#"<script>
(function () {
  var toggle = document.getElementById('rail-toggle');
  var backdrop = document.getElementById('rail-backdrop');
  var rail = document.getElementById('rail');
  if (!toggle || !backdrop || !rail) return;
  function set(open) {
    document.body.classList.toggle('rail-open', open);
    toggle.setAttribute('aria-expanded', open ? 'true' : 'false');
  }
  toggle.addEventListener('click', function () {
    set(!document.body.classList.contains('rail-open'));
  });
  backdrop.addEventListener('click', function () { set(false); });
  rail.addEventListener('click', function (e) {
    if (e.target.closest('.rail__link')) set(false);
  });
  document.addEventListener('keydown', function (e) {
    if (e.key === 'Escape') set(false);
  });
})();
</script>"#
            .to_string(),
    )
}

fn rail_link(base: &str, href: &str, label: &str, ico: icondata::Icon, active: bool) -> Markup {
    html! {
        a.rail__link.is-active[active] href={ (base) (href) } {
            span.rail__link-icon { (icon(ico, 16)) }
            span.rail__link-label { (label) }
        }
    }
}

pub fn icon(ico: icondata::Icon, size: u16) -> Markup {
    html! {
        svg
            viewBox=[ico.view_box]
            width=(size) height=(size)
            fill=[ico.fill]
            stroke=[ico.stroke]
            stroke-width=[ico.stroke_width]
            stroke-linecap=[ico.stroke_linecap]
            stroke-linejoin=[ico.stroke_linejoin]
            aria-hidden="true"
        {
            (PreEscaped(ico.data))
        }
    }
}
