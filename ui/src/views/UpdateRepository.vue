<template>
  <v-card
    v-if="repository"
    flat
    style="border-radius: 0;"
  >
    <v-toolbar
      flat
      dark
    >
      <v-toolbar-title>
        Update Repository
      </v-toolbar-title>
      <v-spacer />
      <v-toolbar-items>
        <v-btn @click="onSave">
          Save
          <v-icon right small>fas fa-save</v-icon>
        </v-btn>
      </v-toolbar-items>
    </v-toolbar>
    <v-container
      fluid
      class="grey lighten-4 fill-height"
    >
      <v-row>
        <v-col>
          <v-text-field
            v-model="repository.id"
            label="ID"
            outlined
            readonly
            disabled
          ></v-text-field>
        </v-col>
      </v-row>
      <v-row>
        <v-col>
          <v-text-field
            v-model="repository.name"
            label="Name"
            outlined
          ></v-text-field>
        </v-col>
      </v-row>
      <v-row>
        <v-col>
          <v-text-field
            v-model="repository.slug"
            label="Slug"
            outlined
            readonly
            disabled
          ></v-text-field>
        </v-col>
      </v-row>
      <v-row>
        <v-col>
          <v-text-field
            v-model="repository.run"
            label="Run Command"
            outlined
          ></v-text-field>
        </v-col>
      </v-row>
      <v-row>
        <v-col>
          <v-text-field
            v-model="repository.working_dir"
            label="Working Directory"
            outlined
          ></v-text-field>
        </v-col>
      </v-row>
    </v-container>
  </v-card>
</template>

<script lang="ts">
import Vue from 'vue'
import Component from 'vue-class-component'
import {state, Repository} from '../store/state'

@Component({
  name: 'UpdateRepository',
})
export default class UpdateRepository extends Vue {
  state = state

  repository: Repository | null = null

  async mounted() {
    this.repository = await this.state.getRepository(this.$route.params.repository)
  }

  async onSave() {
    if (this.repository !== null) {
      await this.state.saveRepository(this.repository)
      this.$router.push(`/manage/repositories`)
    }
  }
}
</script>

