import {
  ExecuteResult,
  InstantiateOptions,
  InstantiateResult,
  SigningCosmWasmClient,
  UploadResult,
} from "@cosmjs/cosmwasm-stargate";
import {
  calculateFee,
  GasPrice,
  IndexedTx,
  MsgTransferEncodeObject,
  SigningStargateClient,
} from "@cosmjs/stargate";
import {
  Coin,
  coin,
  DirectSecp256k1HdWallet,
  EncodeObject,
} from "@cosmjs/proto-signing";

export interface Chain {
  wallet: DirectSecp256k1HdWallet;
  client: SigningStargateClient;
  gasPrice: GasPrice;

  accountAddress: () => Promise<string>;
  nextBlock: () => Promise<number>;
  ibcTransfer: (
    channel: string,
    recipient: string,
    amount: number,
    denom: string,
  ) => Promise<IndexedTx>;
  balanceOf: (account: string, denom: string) => Promise<bigint | null>;
  totalDelegations: (account: string) => Promise<bigint | null>;
}

interface Connect {
  connect: (endpoint: string) => Promise<ConnectedChain>;
}

export interface Contract {
  address: string;
  query: (msg: any) => Promise<any>;
  execute: (
    msg: any,
    from: string,
    gas: number,
    funds?: Coin,
  ) => Promise<ExecuteResult>;
}

export interface CosmWasmChain extends Chain {
  cwClient: SigningCosmWasmClient;

  uploadWasm: (wasmBytes: Uint8Array) => Promise<[number, UploadResult]>;

  initContract: (
    codeId: number,
    msg: any,
    label: string,
    funds?: number,
    admin?: string,
  ) => Promise<[Contract, InstantiateResult]>;

  existingContract: (
    address: string
  ) => Contract;
}

class ConnectedChain implements Chain {
  wallet: DirectSecp256k1HdWallet;
  client: SigningStargateClient;
  gasPrice: GasPrice;
  _accountAddress: string | null;

  constructor(
    wallet: DirectSecp256k1HdWallet,
    client: SigningStargateClient,
    gasPrice: GasPrice,
  ) {
    this.wallet = wallet;
    this.client = client;
    this.gasPrice = gasPrice;
    this._accountAddress = null;
  }

  async waitForTx(id: string): Promise<IndexedTx> {
    while (true) {
      const tx = await this.client.getTx(id);

      if (tx) return tx;

      await this.nextBlock();
    }
  }

  async sendMsgs(msgs: EncodeObject[], gas: number): Promise<IndexedTx> {
    const sender = await this.accountAddress();

    const fee = calculateFee(gas, this.gasPrice);

    const deliverTx = await this.client.signAndBroadcast(sender, msgs, fee);

    return await this.waitForTx(deliverTx.transactionHash);
  }

  async accountAddress(): Promise<string> {
    if (this._accountAddress) return this._accountAddress;

    const accounts = await this.wallet.getAccounts();

    const address = accounts[0].address;

    this._accountAddress = address;

    return address;
  }

  async nextBlock(): Promise<number> {
    const startBlock = await this.client.getBlock();

    while (true) {
      await Bun.sleep(1000);

      const currentBlock = await this.client.getBlock();

      if (currentBlock.header.height > startBlock.header.height) {
        return currentBlock.header.height;
      }
    }
  }

  async ibcTransfer(
    channel: string,
    recipient: string,
    amount: number,
    denom: string,
  ): Promise<IndexedTx> {
    const sender = await this.accountAddress();

    const token = coin(amount, denom);

    const timeoutTimestamp: bigint = BigInt(
      (Date.now() + (5 * 60 * 60 * 1000)) * 1e6,
    );

    const transferMsg: MsgTransferEncodeObject = {
      typeUrl: "/ibc.applications.transfer.v1.MsgTransfer",
      value: {
        sourcePort: "transfer",
        sourceChannel: channel,
        sender,
        token,
        receiver: recipient,
        timeoutTimestamp,
      },
    };

    return this.sendMsgs([transferMsg], 500000);
  }

  async balanceOf(account: string, denom: string): Promise<bigint | null> {
    const coin = await this.client.getBalance(account, denom);

    if (coin) return BigInt(coin.amount);

    return null;
  }

  async totalDelegations(account: string): Promise<bigint | null> {
    const delegations = await this.client.getBalanceStaked(account);

    if (delegations) return BigInt(delegations.amount);

    return null;
  }
}

class CosmWasmContract implements Contract {
  address: string;
  client: SigningCosmWasmClient;
  gasPrice: GasPrice;

  constructor(
    address: string,
    client: SigningCosmWasmClient,
    gasPrice: GasPrice,
  ) {
    this.address = address;
    this.client = client;
    this.gasPrice = gasPrice;
  }

  async query(msg: any): Promise<any> {
    return this.client.queryContractSmart(this.address, msg);
  }

  async execute(
    msg: any,
    from: string,
    gas: number,
    funds?: Coin,
  ): Promise<ExecuteResult> {
    const coins = funds ? [funds] : [];

    const fee = calculateFee(gas, this.gasPrice);

    return this.client.execute(from, this.address, msg, fee, "", coins);
  }
}

class ConnectedCosmWasmChain extends ConnectedChain implements CosmWasmChain {
  cwClient: SigningCosmWasmClient;

  constructor(
    wallet: DirectSecp256k1HdWallet,
    sgClient: SigningStargateClient,
    cwClient: SigningCosmWasmClient,
    gasPrice: GasPrice,
  ) {
    super(wallet, sgClient, gasPrice);

    this.cwClient = cwClient;
  }

  async uploadWasm(wasmBytes: Uint8Array): Promise<[number, UploadResult]> {
    const sender = await this.accountAddress();

    const fee = calculateFee(5000000, this.gasPrice);

    const uploadResult = await this.cwClient.upload(sender, wasmBytes, fee);

    return [uploadResult.codeId, uploadResult];
  }

  async initContract(
    codeId: number,
    msg: any,
    label: string,
    funds?: number | null,
    admin?: string,
  ): Promise<[Contract, InstantiateResult]> {
    const sender = await this.accountAddress();

    const fee = calculateFee(1500000, this.gasPrice);

    const options: InstantiateOptions = {
      funds: funds ? [coin(funds, this.gasPrice.denom)] : undefined,
      admin
    };

    const initResult = await this.cwClient.instantiate(
      sender,
      codeId,
      msg,
      label,
      fee,
      options as InstantiateOptions,
    );

    const contract = new CosmWasmContract(
      initResult.contractAddress,
      this.cwClient,
      this.gasPrice,
    );

    return [contract, initResult];
  }

  existingContract(
    address: string
  ): Contract {
    return new CosmWasmContract(
      address,
      this.cwClient,
      this.gasPrice,
    );
  }
}

class DisconnectedChain {
  wallet: DirectSecp256k1HdWallet;
  gasPrice: GasPrice;

  constructor(
    wallet: DirectSecp256k1HdWallet,
    gasPrice: GasPrice,
  ) {
    this.wallet = wallet;
    this.gasPrice = gasPrice;
  }

  protected static async createInstance<T extends DisconnectedChain>(
    this: new (wallet: DirectSecp256k1HdWallet, gasPrice: GasPrice) => T,
    prefix: string,
    mnemonic: string,
    gasPriceStr: string,
  ): Promise<T> {
    const wallet = await DirectSecp256k1HdWallet.fromMnemonic(mnemonic, {
      prefix,
    });
    const gasPrice = GasPrice.fromString(gasPriceStr);
    return new this(wallet, gasPrice);
  }
}

export class HostChain extends DisconnectedChain implements Connect {
  static async create(
    prefix: string,
    mnemonic: string,
    gasPriceStr: string,
  ): Promise<HostChain> {
    return this.createInstance(prefix, mnemonic, gasPriceStr);
  }

  async connect(endpoint: string): Promise<ConnectedCosmWasmChain> {
    const cwClient: SigningCosmWasmClient = await SigningCosmWasmClient
      .connectWithSigner(
        endpoint,
        this.wallet,
      );

    const sgClient: SigningStargateClient = await SigningStargateClient
      .connectWithSigner(
        endpoint,
        this.wallet,
      );

    await sgClient.getBlock();

    return new ConnectedCosmWasmChain(
      this.wallet,
      sgClient,
      cwClient,
      this.gasPrice,
    );
  }
}

export class RemoteChain extends DisconnectedChain implements Connect {
  static async create(
    prefix: string,
    mnemonic: string,
    gasPriceStr: string,
  ): Promise<RemoteChain> {
    return this.createInstance(prefix, mnemonic, gasPriceStr);
  }

  async connect(endpoint: string): Promise<ConnectedChain> {
    const client: SigningStargateClient = await SigningStargateClient
      .connectWithSigner(
        endpoint,
        this.wallet,
      );

    await client.getBlock();

    return new ConnectedChain(this.wallet, client, this.gasPrice);
  }
}

