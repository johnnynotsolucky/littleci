<template>
  <v-card flat style="border-radius: 0;">
    <v-toolbar
      flat
      dark
    >
      <v-toolbar-title>
        Manage Repositories
      </v-toolbar-title>
      <v-spacer />
      <v-toolbar-items>
        <v-btn to="/manage/repositories/new">
          New
          <v-icon right small>fas fa-plus</v-icon>
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
            :items="state.repositories"
            :items-per-page="15"
            v-if="state.repositories"
          >
            <template v-slot:item.id="{ item }">
              {{ item.id }}
            </template>
            <template v-slot:item.status="{ item }">
              <v-chip dark>{{ item.status }}</v-chip>
            </template>
            <template v-slot:item.action="{ item }">
              <v-btn class="mr-2" text icon :to="`/manage/repositories/${item.slug}`">
                <v-icon small>fas fa-edit</v-icon>
              </v-btn>
              <v-btn class="mr-2" text icon @click="deleteRepository(item.id)">
                <v-icon small>fas fa-trash</v-icon>
              </v-btn>
            </template>
          </v-data-table>
        </v-layout>
      </v-row>
    </v-container>
  </v-card>
</template>

<script lang="ts">
import Vue from 'vue'
import Component from 'vue-class-component'
import {state} from '../store/state'

@Component({
  name: 'ManageRepositories',
})
export default class ManageRepositories extends Vue {
  state = state

  get headers(): any[] {
    return [{
        text: 'ID',
        align: 'left',
        sortable: false,
        value: 'id',
      },
      {
        text: 'Name',
        align: 'left',
        sortable: false,
        value: 'name',
      },
      {
        text: 'Slug',
        align: 'left',
        sortable: false,
        value: 'slug',
      },
      {
        sortable: false,
        value: 'action',
      },
    ]
  }

  async mounted() {
    await this.state.getRepositories()
  }

  async deleteRepository(id: string) {
    await this.state.deleteRepository(id)
    await this.state.getRepositories()
  }
}
</script>
