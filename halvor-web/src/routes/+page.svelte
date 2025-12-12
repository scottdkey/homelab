<script lang="ts">
  import { onMount } from 'svelte';
  import { api, type DiscoveredHost } from '$lib/api';

  let hosts: DiscoveredHost[] = [];
  let loading = false;
  let error: string | null = null;

  async function discoverAgents() {
    loading = true;
    error = null;

    try {
      // Call Rust API server
      hosts = await api.discoverAgents();
    } catch (e) {
      error = e instanceof Error ? e.message : 'Unknown error';
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    // Optionally auto-discover on mount
  });
</script>

<svelte:head>
  <title>Halvor - Agent Discovery</title>
</svelte:head>

<main>
  <h1>Halvor Agent Discovery</h1>

  <button on:click={discoverAgents} disabled={loading}>
    {loading ? 'Discovering...' : 'Discover Agents'}
  </button>

  {#if error}
    <div class="error">{error}</div>
  {/if}

  {#if hosts.length > 0}
    <ul>
      {#each hosts as host}
        <li>
          <strong>{host.hostname}</strong>
          {#if host.localIp}
            - {host.localIp}
          {/if}
          {host.reachable ? '✓' : '✗'}
        </li>
      {/each}
    </ul>
  {/if}
</main>

<style>
  main {
    padding: 2rem;
    max-width: 800px;
    margin: 0 auto;
  }

  button {
    padding: 0.5rem 1rem;
    font-size: 1rem;
    cursor: pointer;
  }

  .error {
    color: red;
    margin-top: 1rem;
  }

  ul {
    list-style: none;
    padding: 0;
    margin-top: 2rem;
  }

  li {
    padding: 0.5rem;
    border-bottom: 1px solid #eee;
  }
</style>
