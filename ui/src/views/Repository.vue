<template>
  <v-card flat style="border-radius: 0;">
    <v-toolbar
      flat
      dark
    >
      <v-toolbar-title>{{ repository && repository.name }}</v-toolbar-title>
      <v-spacer />
      <v-toolbar-items>
        <v-btn :to="`/manage/repositories/${repository.slug}`">
          Edit
          <v-icon right small>fas fa-edit</v-icon>
        </v-btn>
        <v-btn @click="onBuild">
          Build
          <v-icon right small>fas fa-clock</v-icon>
        </v-btn>
      </v-toolbar-items>
    </v-toolbar>
    <v-container
      fluid
      class="grey lighten-4 fill-height"
    >
      <v-row>
        <v-layout child-flex>
          <v-data-table
            :headers="headers"
            :items="jobs"
            :items-per-page="15"
            @click:row="onItemClick"
            v-if="jobs"
          >
            <template v-slot:item.id="{ item }">
              {{ item.id }}
            </template>
            <template v-slot:item.status="{ item }">
              <v-chip dark>{{ item.status }}</v-chip>
            </template>
          </v-data-table>
        </v-layout>
      </v-row>
    </v-container>
  </v-card>
</template>

<script lang="ts">
import Vue from 'vue'
import { Component, Watch } from 'vue-property-decorator'
import {state, Job, Repository as RepositoryType} from '../store/state'

@Component({
  name: 'Repository',
})
export default class Repository extends Vue {
  private name!: string
  private state = state
  private slug!: string
  private repository: RepositoryType | null = null
  private jobs: Job[] = []

  @Watch('$route', { immediate: true })
  async onUrlChanged(to: any) {
    this.slug = to.params.repository
    if (this.slug) {
      [this.repository, this.jobs] = await Promise.all([
        await this.state.getRepository(this.slug),
        await this.state.getRepositoryJobs(this.slug),
      ])
    }
  }

  get headers(): any[] {
    return [{
        text: 'Job',
        align: 'left',
        sortable: false,
        value: 'id',
      },
      {
        text: 'Time',
        align: 'left',
        sortable: false,
        value: 'updated_at',
      },
      {
        text: 'Status',
        align: 'right',
        sortable: false,
        value: 'status',
      },
    ]
  }

  onItemClick(item: Job) {
    this.$router.push(`/repositories/${this.slug}/jobs/${item.id}`)
  }
}
</script>

