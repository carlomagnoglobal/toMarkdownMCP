---
author: Addy Osmani
description: You don't really need to be good at prompting anymore. The thing to get good at is the loop that does the prompting for you. It's five building blocks plus s...
language: en
msapplication-tilecolor: #ffc40d
msapplication-tileimage: /assets/images/favicons/mstile-144x144.png
og-description: You don't really need to be good at prompting anymore. The thing to get good at is the loop that does the prompting for you. It's five building blocks plus s...
og-locale: en_US
og-title: Loop Engineering
theme-color: #eb298c
title: AddyOsmani.com - Loop Engineering
twitter:card: summary_large_image
twitter:creator: @addyosmani
twitter:description: You don't really need to be good at prompting anymore. The thing to get good at is the loop that does the prompting for you. It's five building blocks plus s...
twitter:image: "https://addyosmani.com/assets/images/loop-engineering.jpg"
twitter:image:src: "https://addyosmani.com/assets/images/loop-engineering.jpg"
twitter:site: @addyosmani
twitter:title: Loop Engineering
twitter:url: "https://addyosmani.com/"
viewport: width=device-width, initial-scale=1
---

# AddyOsmani.com - Loop Engineering

/goal does under the hood, a fresh model decides if the loop is done instead of the one that did the work, the maker and checker split applied to the stop condition itself.

What one loop looks like
----------

Stick it together and a single thread turns into a little control panel. Here is one shape I keep using.

An automation runs every morning on the repo. Its prompt calls a triage skill that reads yesterdays CI failures, the open issues, the recent commits, and writes the findings into a markdown file or a Linear board. For each finding that is worth doing the thread opens an isolated worktree and sends a sub-agent to draft the fix, and a second sub-agent reviews that draft against the project skills and the existing tests.

Connectors let the loop open the PR and update the ticket. Anything the loop can not handle lands in the triage inbox for me. The state file is the spine of the whole thing, it remembers what got tried, what passed, what is still open, so tomorrow morning the run picks up where today stopped.

And look at what you actually did there. You designed it one time. You did not prompt any of those steps. Thats Steinberger’s whole point made real, and its the same loop in Codex or in Claude Code because the pieces are the same pieces.

What the loop still does not do for you
----------

The loop changes the work, it does not delete you from it. And three problems actually get sharper as the loop gets better, not easier.

Verification is still on you. A loop running unattended is also a loop making mistakes unattended. The whole reason you split the verifier sub-agent from the maker is to make the loop’s “its done” mean something, and even then “done” is a claim and not a proof. I keep saying the same line from [code review in the age of AI](https://addyosmani.com/blog/code-review-ai/), your job is to ship code you confirmed works.

Your understanding still rots if you allow it. The faster the loop ships code you did not write, the bigger the gap between what exists and what you actually get. Thats [comprehension debt](https://addyosmani.com/blog/comprehension-debt/) and a smooth loop just makes it grow faster unless you read what the loop made.

And the comfortable posture is the dangerous one. When the loop runs itself its very tempting to stop having an opinion and just take whatever it gives back. I called that [cognitive surrender](https://addyosmani.com/blog/cognitive-surrender/). Designing the loop is the cure when you do it with judgement and the accelerant when you do it to avoid thinking, same action, opposite result.

Build the loop. Stay the engineer.
----------

I think this is a preview of how our work is going to evolve. That said, If I weren’t reviewing the code myself or if I relied entirely on automated loops to fix it my product’s quality would suffer. I’d likely end up stuck in a downward spiral, continuously digging myself into a deeper hole.

That said, go ahead and set up your loops, but don’t forget that prompting your agents directly is also effective. It’s all about finding the right balance.

Loops can also result in different outcomes depending on you. Two people can build the exact same loop and get completely opposite results. One uses it to move faster on work they understand deeply. The other uses it to avoid understanding the work at all. The loop doesn’t know the difference. You do.

That’s what makes loop design harder than prompt engineering, not easier. Cherny’s point isn’t that the work got easier. It’s that the leverage point moved.

Build the loop. But build it like someone who intends to stay the engineer, not just the person who presses go.

[<img width="120" alt="Beyond Vibe Coding book cover" loading="lazy" src="https://addyosmani.com/assets/images/beyond.webp"> ](https://beyond.addy.ie)

Enjoyed this?

### Go deeper in *Beyond Vibe Coding* ###

My O'Reilly book on AI-assisted and agentic engineering: specs, harnesses, evals, context, and shipping production-grade software with AI.

[Read the book](https://beyond.addy.ie)

[![](/assets/images/addy_2022.jpg)

Addy Osmani is an engineering and evangelism leader who spent over 14 years at Google leading developer experience across Chrome and, in recent years, AI (Gemini, coding agents, and agentic engineering), most recently as a Director at Google Cloud AI.

](http://twitter.com/addyosmani)

[ Tweet](https://twitter.com/intent/tweet?text=https://addyosmani.com/blog/loop-engineering/ - Loop Engineering by @addyosmani) [ Bluesky](https://bsky.app/intent/compose?text=Loop Engineering - https://addyosmani.com/blog/loop-engineering/) [ Mastodon](https://mastodon.social/share?text=Loop Engineering%0Ahttps://addyosmani.com/blog/loop-engineering/) [ Threads](https://www.threads.net/intent/post?text=Loop Engineering%0Ahttps://addyosmani.com/blog/loop-engineering/) [ LinkedIn](https://www.linkedin.com/sharing/share-offsite/?url=https://addyosmani.com/blog/loop-engineering/) [ Share](#)

Want more? Subscribe to my free newsletter:

Subscribe

**Disclaimer:** The views and opinions expressed on this site are my own and do not necessarily reflect the views, positions, or strategies of Google or any of its affiliates.

© Copyright 2026 Addy Osmani
