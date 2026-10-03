<script lang="ts">
  import Section from "$lib/components/Section.svelte";
  import Row from "$lib/components/Row.svelte";
</script>

<Section title="Installation">
  <p>
    This app needs a helper called framemate-agent, a small service running on the Frame. <br>
    The agent is shipped as a Flatpak that you will need to install on your Steam Frame. The easiest way to install it is through SSH if you have that setup to your Frame, otherwise the Desktop Mode Terminal will also work. <br>
    <b>Check out the GitHub page for exact install instructions</b>
  </p>
</Section>

<Section title="Token">
  <p>
    FrameMate uses a token for communicating with your Steam Frame. This token is show on installation or by running
    <code>flatpak run dev.framemate.Agent token</code> on the headset with framemate-agent installed.
    Write it down, you will need it to connect the app with the headset.
  </p>
</Section>

<Section title="Connecting">
  <p>
    Once the agent is installed on the headset, you aquired the token and the app is ready on your phone you can connect the two.
    The connection requires the Frame's address, usually this will be frame.local, if that doesn't work try your headsets IP address directly.
    You can find out your IP by running <code>ip a</code>. Both headset and phone need to be in the same local network for the connection to work.
  </p>
</Section>

<Section title="Issues">
  <p>
    If you encounter any issues with the app or agent, please submit a GitHub Issue so I can get it fixed. <br>
    <a href="https://github.com/nailuj05/framemate/issues/new">Submit an Issue here</a>
  </p>
</Section>

<style>
 p {
   gap: 12px;
   min-height: 48px;
   padding: 8px var(--gutter);
   background: var(--surface);
   border-bottom: 1px solid var(--divider);
   color: var(--text);
 }
 a {
   color: #0000EE;
 }
</style>
