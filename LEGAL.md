# Recording other people

This program records audio. Some of that audio is other people's voices, and in
much of the world recording a person speaking is regulated — sometimes by
privacy law, sometimes by criminal law, usually by your employment contract as
well.

**None of what follows is legal advice**, and **the author of this program
accepts no responsibility for what you do with it.** What follows is a
description of the questions that tend to matter, written so that you know
which ones to ask. If a recording could end up in a dispute, a grievance, a
disciplinary process or a court, ask a lawyer rather than a README.

See [No warranty, and no responsibility for use](#no-warranty-and-no-responsibility-for-use)
for the position in full.

## The short version

Before you record a meeting, a call, or a remote session that other people are
speaking in:

1. **Tell them.** Say it out loud at the start, and let it be minuted. Most of
   the rules that follow are satisfied by the participants knowing.
2. **Check your employer's policy.** It frequently prohibits recording
   regardless of what the law allows, and it is the rule most likely to be
   enforced against you.
3. **Check whether the other party is a customer, a patient, a minor, or a
   regulated relationship.** These attract additional duties.
4. **Decide where the file will live and for how long.** A recording you cannot
   account for is a liability rather than an asset.

If you cannot do the first one, that is usually the answer.

## Consent

Jurisdictions differ, and the differences are the point.

**One-party consent** means one participant's consent is enough, and if you are
in the conversation, that can be you. Most of the United States federally and at
state level, and Canada, work this way.

**All-party consent** means every participant must agree. In the United States
this includes California, Florida, Illinois, Maryland, Massachusetts, Michigan,
Montana, Nevada, New Hampshire, Oregon, Pennsylvania and Washington, and the
list is neither stable nor exhaustive. Recording a call with someone in an
all-party state can bring you under their rules even if you are not in one, and
several of these are criminal statutes rather than civil ones.

**The European Union and the United Kingdom** approach it through data
protection rather than wiretapping. A voice recording is personal data, and
processing it needs a lawful basis, a stated purpose, a retention period, and a
way for the people recorded to exercise their rights. Consent is one lawful
basis; "legitimate interests" is another and is harder to rely on than it looks.
Recording without telling anyone is very difficult to defend under either.

**Germany** is worth naming separately: § 201 StGB makes recording the
non-public spoken word of another person a criminal offence, and consent is the
exception rather than the rule.

**Elsewhere** varies widely. Do not assume the rules you know travel with you.

## Meetings and calls at work

Two things are commonly true at once: the law permits the recording, and your
employer forbids it. The second is what you will be judged by.

Corporate policy, an acceptable-use agreement, or the terms of the conferencing
platform frequently prohibit recording, or permit it only through the
platform's own recording feature — which announces itself to participants. A
tool that records the audio arriving at your machine deliberately does not
announce anything, which is exactly why using it in place of the platform's own
recording can be a disciplinary matter even where it is lawful.

If you are recording because you do not trust the other participants, you are in
the situation where the rules matter most.

## Remote sessions and virtual desktops

This program exists to capture audio from Citrix HDX sessions, virtual desktops
and WSL — audio that originates somewhere else and passes through your machine.
That raises two extra questions.

The audio may belong to a system whose data you are permitted to *use* but not
to *extract*. Recording a remote session can be exfiltration in the eyes of the
organisation that granted the session, whatever the audio contains. Some
environments deploy technical controls against exactly this, and defeating or
circumventing such a control is a separate and more serious matter than the
recording itself. This program does not attempt to.

And the remote session may be governed by a contract you have not read, between
your employer and a client. Client data is usually the most tightly held.

## Special categories

Some content attracts heavier duties, and it is often not obvious in advance
that a call will contain it:

- **Health information** about an identifiable person.
- **Children**, where consent is generally the guardian's to give.
- **Legal advice**, which can carry privilege that a recording may waive.
- **Payment card details** read aloud, which fall under PCI DSS. A recording
  containing a card number and its security code is a compliance incident in
  itself.
- **Trade secrets and material non-public information**, where the recording
  becomes a record you may be obliged to retain and disclose.

## Keeping and deleting

A recording is personal data about everyone audible in it, for as long as it
exists.

- Decide the retention period before you record, and delete on schedule.
- Keep recordings where you keep other confidential material, not in a synced
  folder that propagates them to places you have not thought about.
- Note that this program writes uncompressed audio, so a session is large and
  conspicuous, which cuts both ways.
- If someone asks whether you hold a recording of them, in the EU or the UK you
  will generally have to answer, and possibly produce it.

The program itself is designed to keep this simple: recordings are written where
you tell it to and nowhere else, nothing is uploaded, and nothing about a session
leaves the machine. See [PRIVACY.md](PRIVACY.md).

## Transcription

The transcription feature runs a speech engine installed on your own machine, so
the audio is not sent anywhere. If you replace it with a cloud service, the audio
and the transcript become a transfer to a third party, and everything above
applies again to that transfer — including, for EU and UK data, where the
processing happens.

A transcript is often more sensitive than the audio, because it is searchable.

## No warranty, and no responsibility for use

This program is a tool. It is distributed under the [MIT License](LICENSE),
which disclaims all warranties and limits liability, and that disclaimer is
meant seriously rather than as a formality.

In plain terms:

- **The author accepts no responsibility or liability for how this program is
  used**, for anything recorded with it, or for any consequence of that
  recording — legal, disciplinary, contractual, reputational or financial.
- **No representation is made that any particular use is lawful.** Whether
  recording a given conversation is permitted where you are, and whether it is
  allowed by your employer, your client, or the terms of the service you are
  recording, is for you to establish. This program does not know where you are
  or what you have agreed to.
- **Deciding what may be recorded is yours.** So is obtaining whatever consent
  is required, telling the people involved, and looking after the files
  afterwards.
- **You are solely responsible for compliance** with every law, regulation,
  policy, contract and professional obligation that applies to you. If a
  recording made with this program breaks any of them, that is your act and
  your responsibility, not the author's.
- **The program is provided as is**, with no warranty of merchantability,
  fitness for a particular purpose, accuracy, or completeness. It may lose
  audio, mis-time it, or fail to record at all; do not rely on it where a
  failed recording would cause harm.
- To the maximum extent permitted by applicable law, the author shall not be
  liable for any claim, damages, loss or other liability, whether in an action
  of contract, tort or otherwise, arising from or in connection with this
  program or its use.
- Nothing in this document or anywhere else in this project creates a
  lawyer–client relationship or constitutes legal advice.

If you are not willing to carry that responsibility yourself, do not use this
program to record other people.

## If you are on the other side of this

If you believe a recording of you was made with this program, the author holds
no recordings and no logs: the program has no server component and reports
nothing to anyone. Whoever ran it holds the files. See
[PRIVACY.md](PRIVACY.md) for what the program does and does not do.
